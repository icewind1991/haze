use crate::database::Database;
use crate::image::pull_image;
use crate::network::ensure_network_exists;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::network::ConnectNetworkOptions;
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Report, Result};
use maplit::hashmap;
use reqwest::{Client, Url};
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum PhpVersion {
    Php80,
    Php74,
    Php73,
    Php72,
}

impl FromStr for PhpVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "7" => Ok(PhpVersion::Php74),
            "7.2" => Ok(PhpVersion::Php72),
            "7.3" => Ok(PhpVersion::Php73),
            "7.4" => Ok(PhpVersion::Php74),
            "8" => Ok(PhpVersion::Php80),
            "8.0" => Ok(PhpVersion::Php80),
            _ => Err(()),
        }
    }
}

impl PhpVersion {
    fn image(&self) -> &'static str {
        // for now only 7.4
        match self {
            PhpVersion::Php72 => "icewind1991/haze:7.2",
            PhpVersion::Php73 => "icewind1991/haze:7.3",
            PhpVersion::Php74 => "icewind1991/haze:7.4",
            PhpVersion::Php80 => "icewind1991/haze:8.0",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PhpVersion::Php72 => "7.2",
            PhpVersion::Php73 => "7.3",
            PhpVersion::Php74 => "7.4",
            PhpVersion::Php80 => "8.0",
        }
    }

    pub async fn spawn(
        &self,
        docker: &mut Docker,
        id: &str,
        env: Vec<String>,
        db: &Database,
        network: &str,
        volumes: Vec<String>,
        host: &str,
    ) -> Result<String> {
        ensure_network_exists(docker, "haze").await?;
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: id.to_string(),
        });
        let config = Config {
            image: Some(self.image().to_string()),
            env: Some(env),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                binds: Some(volumes),
                extra_hosts: Some(vec![format!("hazehost:{}", host)]),
                ..Default::default()
            }),
            networking_config: Some(NetworkingConfig {
                endpoints_config: hashmap! {
                    network.to_string() => EndpointSettings {
                        aliases: Some(vec!["cloud".to_string()]),
                        ..Default::default()
                    }
                },
            }),
            labels: Some(hashmap! {
                "haze-type".to_string() => "cloud".to_string(),
                "haze-db".to_string() => db.name().to_string(),
                "haze-php".to_string() => self.name().to_string(),
                "haze-cloud-id".to_string() => id.to_string(),
            }),
            ..Default::default()
        };

        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;

        docker
            .connect_network(
                "haze",
                ConnectNetworkOptions {
                    container: id.as_str(),
                    endpoint_config: EndpointSettings {
                        aliases: Some(vec![id.to_string()]),
                        ..Default::default()
                    },
                },
            )
            .await?;

        Ok(id)
    }

    pub async fn wait_for_start(&self, ip: Option<IpAddr>) -> Result<()> {
        let client = Client::new();
        let url = Url::parse(&format!(
            "http://{}/status.php",
            ip.ok_or(Report::msg("Container not running"))?
        ))?;
        timeout(Duration::from_secs(5), async {
            while !client.get(url.clone()).send().await.is_ok() {
                sleep(Duration::from_millis(100)).await
            }
        })
        .await
        .wrap_err("Timeout after 5 seconds")
    }
}

impl Default for PhpVersion {
    fn default() -> Self {
        PhpVersion::Php80
    }
}
