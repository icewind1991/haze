use crate::Result;
use miette::{miette, IntoDiagnostic};
use std::fs::read_dir;
use std::path::Path;
use std::process::Command;
use tracing::error;

pub fn checkout_all<P: AsRef<Path>>(sources_root: P, branch: &str) -> Result<()> {
    let apps_dir = sources_root.as_ref().join("apps");
    for app in read_dir(apps_dir).into_diagnostic()? {
        let app = app.into_diagnostic()?;
        if app.metadata().into_diagnostic()?.is_dir() {
            let app_dir = app.path();
            if has_branch(&app_dir, &branch).unwrap_or_default()
                && get_branch(&app_dir).unwrap_or_default().trim() != branch
            {
                print!("{}", app.file_name().to_string_lossy());
                if let Err(e) = checkout(&app_dir, &branch) {
                    println!(": {} ❌", e);
                } else {
                    println!(" ✓");
                }
            }
        }
    }
    Ok(())
}

fn has_branch<P: AsRef<Path>>(repo: P, branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(repo)
        .arg("branch")
        .arg("-a")
        .output()
        .into_diagnostic()?;
    let stdout = String::from_utf8(output.stdout).into_diagnostic()?;
    let stderr = String::from_utf8(output.stderr).into_diagnostic()?;
    if output.status.code().unwrap_or_default() != 0 {
        error!(stdout = stdout, stderr = stderr, "git command failed");
    }

    for line in stdout.split("\n") {
        let line = line.trim();
        if line == branch {
            return Ok(true);
        }
        if let Some((_, part)) = line.rsplit_once('/') {
            if part == branch {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn checkout<P: AsRef<Path>>(repo: P, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repo)
        .arg("checkout")
        .arg(branch)
        .output()
        .into_diagnostic()?;

    let stdout = String::from_utf8(output.stdout).into_diagnostic()?;
    let stderr = String::from_utf8(output.stderr).into_diagnostic()?;
    let code = output.status.code().unwrap_or_default();
    if code != 0 {
        if stderr.contains("would be overwritten") {
            return Err(miette!("uncommited changes"));
        }
        error!(
            stdout = stdout,
            stderr = stderr,
            code = code,
            "git command failed"
        );
    }

    Ok(())
}

fn get_branch<P: AsRef<Path>>(repo: P) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .into_diagnostic()?;
    String::from_utf8(output.stdout).into_diagnostic()
}
