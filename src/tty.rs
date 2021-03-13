use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};
use std::io::{stdout, Read, Write};
use std::time::Duration;
use termion::async_stdin;
use termion::raw::IntoRawMode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::spawn;
use tokio::time::sleep;

pub async fn exec_tty(
    docker: &mut Docker,
    container: &str,
    user: &str,
    cmd: Vec<String>,
) -> Result<()> {
    let config = CreateExecOptions {
        cmd: Some(cmd),
        user: Some(user.to_string()),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        attach_stdin: Some(true),
        tty: Some(true),
        ..Default::default()
    };
    let message = docker
        .create_exec(container, config)
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
    Ok(())
}
