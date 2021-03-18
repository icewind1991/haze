use bollard::image::CreateImageOptions;
use bollard::models::CreateImageInfo;
use bollard::Docker;
use color_eyre::Result;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::io::stdout;
use std::io::Write;
use termion::cursor;

pub async fn pull_image(docker: &Docker, image: &str) -> Result<()> {
    if let Err(_) = docker.inspect_image(image).await {
        let mut info_stream = docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );

        let mut bars: HashMap<String, u16> = HashMap::new();

        let stdout = stdout();
        let mut stdout = stdout.lock();
        while let Some(info) = info_stream.next().await {
            let info: CreateImageInfo = info?;
            // dbg!(&info);
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
                        )?;
                    }
                    None => {
                        writeln!(stdout, "{} - {:12} {}", id, status, progress)?;
                        bars.insert(id, bars.len() as u16);
                    }
                }
                stdout.flush()?;
            }
        }
    }
    Ok(())
}
