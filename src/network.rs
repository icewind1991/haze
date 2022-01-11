use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use miette::{IntoDiagnostic, Result, WrapErr};

pub async fn clear_networks(docker: &Docker) -> Result<()> {
    let networks = docker
        .list_networks::<&str>(None)
        .await
        .into_diagnostic()
        .wrap_err("Failed to list docker networks")?;
    for network in networks {
        match network.name.as_deref() {
            Some(name) if name.starts_with("haze-") => {
                docker
                    .remove_network(name)
                    .await
                    .into_diagnostic()
                    .wrap_err("Failed to remove docker network")?;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn get_network_id(docker: &Docker, name: &str) -> Result<Option<String>> {
    let networks = docker
        .list_networks::<&str>(None)
        .await
        .into_diagnostic()
        .wrap_err("Failed to list docker networks")?;
    Ok(networks.into_iter().find_map(|network| {
        if network.name.as_deref() == Some(name) {
            Some(network.id.unwrap())
        } else {
            None
        }
    }))
}

pub async fn ensure_network_exists(docker: &Docker, name: &str) -> Result<String> {
    if let Some(id) = get_network_id(docker, name).await? {
        Ok(id)
    } else {
        Ok(docker
            .create_network(CreateNetworkOptions {
                name,
                check_duplicate: true,
                ..Default::default()
            })
            .await
            .into_diagnostic()?
            .id
            .unwrap())
    }
}
