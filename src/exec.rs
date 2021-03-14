use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};
use futures_util::StreamExt;
use std::io::{stdout, Read, Write};
use std::time::Duration;
use termion::async_stdin;
use termion::raw::IntoRawMode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::spawn;
use tokio::time::sleep;

pub async fn exec_tty<S1: AsRef<str>, S2: Into<String>>(
    docker: &mut Docker,
    container: S1,
    user: &str,
    cmd: Vec<S2>,
    env: Vec<&str>,
) -> Result<i64> {
    let cmd = cmd.into_iter().map(S2::into).collect();
    let env = env.into_iter().map(String::from).collect();
    let config = CreateExecOptions {
        cmd: Some(cmd),
        user: Some(user.to_string()),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        attach_stdin: Some(true),
        tty: Some(true),
        env: Some(env),
        ..Default::default()
    };
    let message = docker
        .create_exec(container.as_ref(), config)
        .await
        .wrap_err("Failed to setup exec")?;
    if let StartExecResults::AttachedTTY {
        mut output,
        mut input,
    } = docker
        .start_exec(&message.id, None, true)
        .await
        .wrap_err("Failed to start exec")?
    {
        // pipe stdin into the docker exec stream input
        spawn(async move {
            let mut stdin = async_stdin().bytes();
            loop {
                if let Some(Ok(byte)) = stdin.next() {
                    input.write(&[byte]).await.ok();
                } else {
                    sleep(Duration::from_nanos(10)).await;
                }
            }
        });

        // set stdout in raw mode so we can do tty stuff
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode()?;

        // pipe docker exec output into stdout
        let mut buff = [0; 128];
        while let Ok(read) = output.read(&mut buff).await {
            if read == 0 {
                break;
            }
            stdout.write(&buff[0..read])?;
            stdout.flush()?;
        }
    } else {
        unreachable!();
    }

    Ok(docker
        .inspect_exec(&message.id)
        .await?
        .exit_code
        .unwrap_or_default())
}

pub async fn exec<S1: AsRef<str>, S2: Into<String>>(
    docker: &mut Docker,
    container: S1,
    user: &str,
    cmd: Vec<S2>,
    env: Vec<&str>,
    mut std_out: Option<impl Write>,
) -> Result<i64> {
    let cmd = cmd.into_iter().map(S2::into).collect();
    let env = env.into_iter().map(String::from).collect();
    let config = CreateExecOptions {
        cmd: Some(cmd),
        user: Some(user.to_string()),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        env: Some(env),
        ..Default::default()
    };
    let message = docker
        .create_exec(container.as_ref(), config)
        .await
        .wrap_err("Failed to setup exec")?;
    if let StartExecResults::Attached { mut output, .. } = docker
        .start_exec(&message.id, None, false)
        .await
        .wrap_err("Failed to start exec")?
    {
        while let Some(Ok(line)) = output.next().await {
            if let Some(std_out) = &mut std_out {
                write!(std_out, "{}", line)?;
            }
        }
    } else {
        unreachable!();
    }

    Ok(docker
        .inspect_exec(&message.id)
        .await?
        .exit_code
        .unwrap_or_default())
}
