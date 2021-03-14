use crate::database::Database;
use crate::image::pull_image;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
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
    Latest,
    // Php80,
    Php74,
    // Php73,
}

impl FromStr for PhpVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "7" => Ok(PhpVersion::Php74),
            "7.4" => Ok(PhpVersion::Php74),
            _ => Err(()),
        }
    }
}

impl PhpVersion {
    fn image(&self) -> &'static str {
        // for now only 7.4
        match self {
            PhpVersion::Latest => "icewind1991/haze:7.4",
            PhpVersion::Php74 => "icewind1991/haze:7.4",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PhpVersion::Latest => "7.4",
            PhpVersion::Php74 => "7.4",
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
    ) -> Result<String> {
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: id.to_string(),
        });
        let config = Config {
            image: Some(self.image().to_string()),
            env: Some(env),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                // links: Some(links),
                binds: Some(volumes),
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
        PhpVersion::Latest
    }
}
