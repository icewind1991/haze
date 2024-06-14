use bollard::image::CreateImageOptions;
use bollard::models::CreateImageInfo;
use bollard::Docker;
use futures_util::StreamExt;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::collections::HashMap;
use std::io::stdout;
use std::io::Write;
use termion::cursor;

pub async fn image_exists(docker: &Docker, image: &str) -> bool {
    docker.inspect_image(image).await.is_ok()
}

pub async fn pull_image(docker: &Docker, image: &str) -> Result<()> {
    if docker.inspect_image(image).await.is_err() {
        println!("Pulling image {}", image);

        let mut info_stream = docker.create_image(
            Some(CreateImageOptions {
                from_image: if image.contains(':') {
                    image.to_string()
                } else {
                    format!("{}:latest", image)
                },
                ..Default::default()
            }),
            None,
            None,
        );

        let mut bars: HashMap<String, u16> = HashMap::new();

        let mut stdout = stdout();
        while let Some(info) = info_stream.next().await {
            let info: CreateImageInfo = info
                .into_diagnostic()
                .wrap_err_with(|| format!("Error while pulling image {}", image))?;
            if let (Some(id), Some(status), Some(progress)) = (info.id, info.status, info.progress)
            {
                match bars.get(&id) {
                    Some(pos) => {
                        let offset = bars.len() as u16 - pos;
                        write!(
                            stdout,
                            "{}{}{} - {:12} {}{}",
                            cursor::Save,
                            cursor::Up(offset),
                            id,
                            status,
                            progress,
                            cursor::Restore
                        )
                        .into_diagnostic()?;
                    }
                    None => {
                        writeln!(stdout, "{} - {:12} {}", id, status, progress)
                            .into_diagnostic()?;
                        bars.insert(id, bars.len() as u16);
                    }
                }
                stdout.flush().into_diagnostic()?;
            }
        }
    }
    Ok(())
}
