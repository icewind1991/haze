use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};

pub async fn clear_networks(docker: &Docker) -> Result<()> {
    let networks = docker
        .list_networks::<&str>(None)
        .await
        .wrap_err("Failed to list docker networks")?;
    for network in networks {
        match network.name.as_deref() {
            Some(name) if name.starts_with("haze-") => {
                docker
                    .remove_network(name)
                    .await
                    .wrap_err("Failed to remove docker network")?;
            }
            _ => {}
        }
    }
    Ok(())
}
