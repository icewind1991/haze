use bollard::container::LogsOptions;
use bollard::exec::{CreateExecOptions, ResizeExecOptions, StartExecResults};
use bollard::Docker;
use futures_util::StreamExt;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use std::io::{stdout, Read, Write};
use std::time::Duration;
use termion::raw::IntoRawMode;
use termion::{async_stdin, is_tty, terminal_size};
use tokio::io::AsyncWriteExt;
use tokio::task::spawn;
use tokio::time::sleep;

pub async fn exec_tty<S1: AsRef<str>, S2: Into<String>>(
    docker: &Docker,
    container: S1,
    user: &str,
    cmd: Vec<S2>,
    env: Vec<&str>,
) -> Result<ExitCode> {
    let stdout = stdout();

    if !is_tty(&stdout) {
        return exec(docker, container, user, cmd, env, Some(stdout)).await;
    }

    let tty_size = terminal_size().into_diagnostic()?;
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
        .into_diagnostic()
        .wrap_err("Failed to setup exec")?;
    if let StartExecResults::Attached {
        mut output,
        mut input,
    } = docker
        .start_exec(&message.id, None)
        .await
        .into_diagnostic()
        .wrap_err("Failed to start exec")?
    {
        docker
            .resize_exec(
                &message.id,
                ResizeExecOptions {
                    height: tty_size.1,
                    width: tty_size.0,
                },
            )
            .await
            .ok();

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
        let mut stdout = stdout.lock().into_raw_mode().into_diagnostic()?;

        // pipe docker exec output into stdout
        while let Some(Ok(output)) = output.next().await {
            stdout
                .write(output.into_bytes().as_ref())
                .into_diagnostic()?;
            stdout.flush().into_diagnostic()?;
        }
    } else {
        unreachable!();
    }

    Ok(docker
        .inspect_exec(&message.id)
        .await
        .into_diagnostic()?
        .exit_code
        .unwrap_or_default()
        .into())
}

pub async fn exec<S1: AsRef<str>, S2: Into<String>>(
    docker: &Docker,
    container: S1,
    user: &str,
    cmd: Vec<S2>,
    env: Vec<&str>,
    mut std_out: Option<impl Write>,
) -> Result<ExitCode> {
    let cmd = cmd.into_iter().map(S2::into).collect();
    let env = env.into_iter().map(String::from).collect();
    let config = CreateExecOptions {
        cmd: Some(cmd),
        user: Some(user.to_string()),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        env: Some(env),
        tty: Some(false),
        ..Default::default()
    };
    let message = docker
        .create_exec(container.as_ref(), config)
        .await
        .into_diagnostic()
        .wrap_err("Failed to setup exec")?;
    if let StartExecResults::Attached { mut output, .. } = docker
        .start_exec(&message.id, None)
        .await
        .into_diagnostic()
        .wrap_err("Failed to start exec")?
    {
        while let Some(Ok(line)) = output.next().await {
            if let Some(std_out) = &mut std_out {
                write!(std_out, "{}", line).into_diagnostic()?;
            }
        }
    } else {
        unreachable!();
    }

    Ok(docker
        .inspect_exec(&message.id)
        .await
        .into_diagnostic()?
        .exit_code
        .unwrap_or_default()
        .into())
}

pub async fn container_logs(
    docker: &Docker,
    mut std_out: impl Write,
    container: &str,
    count: usize,
    follow: bool,
) -> Result<()> {
    let mut stream = docker.logs::<String>(
        container,
        Some(LogsOptions {
            stdout: true,
            stderr: true,
            follow,
            tail: format!("{}", count),
            ..Default::default()
        }),
    );
    while let Some(line) = stream.next().await {
        let line = line.into_diagnostic()?.to_string();
        write!(&mut std_out, "{}", line).into_diagnostic()?;
    }
    Ok(())
}

pub struct ExitCode(i64);

impl ExitCode {
    pub fn to_result(&self) -> Result<()> {
        match self.0 {
            0 => Ok(()),
            code => Err(Report::msg(format!(
                "Command failed with exit code {}",
                code
            ))),
        }
    }
}

impl PartialEq<i64> for ExitCode {
    fn eq(&self, other: &i64) -> bool {
        &self.0 == other
    }
}

impl From<i64> for ExitCode {
    fn from(code: i64) -> Self {
        ExitCode(code)
    }
}
