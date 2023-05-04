use crate::database::Database;
use crate::image::pull_image;
use crate::network::ensure_network_exists;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::network::ConnectNetworkOptions;
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use reqwest::{Client, Url};
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum PhpVersion {
    Php82,
    Php81,
    Php80,
    Php74,
    Php73,
    Php82Dbg,
    Php81Dbg,
    Php80Dbg,
    Php74Dbg,
    Php73Dbg,
}

pub const PHP_MEMORY_LIMIT: i64 = 2 * 1024 * 1024 * 1024;

impl FromStr for PhpVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "7" => Ok(PhpVersion::Php74),
            "7.3" => Ok(PhpVersion::Php73),
            "7.4" => Ok(PhpVersion::Php74),
            "8" => Ok(PhpVersion::Php81),
            "8.0" => Ok(PhpVersion::Php80),
            "8.1" => Ok(PhpVersion::Php81),
            "8.2" => Ok(PhpVersion::Php82),
            "7-dbg" => Ok(PhpVersion::Php74Dbg),
            "7.3-dbg" => Ok(PhpVersion::Php73Dbg),
            "7.4-dbg" => Ok(PhpVersion::Php74Dbg),
            "8-dbg" => Ok(PhpVersion::Php80Dbg),
            "8.0-dbg" => Ok(PhpVersion::Php80Dbg),
            "8.1-dbg" => Ok(PhpVersion::Php81Dbg),
            "8.2-dbg" => Ok(PhpVersion::Php82Dbg),
            _ => Err(()),
        }
    }
}

impl PhpVersion {
    fn image(&self) -> &'static str {
        // for now only 7.4
        match self {
            PhpVersion::Php73 => "icewind1991/haze:7.3",
            PhpVersion::Php74 => "icewind1991/haze:7.4",
            PhpVersion::Php80 => "icewind1991/haze:8.0",
            PhpVersion::Php81 => "icewind1991/haze:8.1",
            PhpVersion::Php82 => "icewind1991/haze:8.2",
            PhpVersion::Php73Dbg => "icewind1991/haze:7.3-dbg",
            PhpVersion::Php74Dbg => "icewind1991/haze:7.4-dbg",
            PhpVersion::Php80Dbg => "icewind1991/haze:8.0-dbg",
            PhpVersion::Php81Dbg => "icewind1991/haze:8.1-dbg",
            PhpVersion::Php82Dbg => "icewind1991/haze:8.2-dbg",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PhpVersion::Php73 => "7.3",
            PhpVersion::Php74 => "7.4",
            PhpVersion::Php80 => "8.0",
            PhpVersion::Php81 => "8.1",
            PhpVersion::Php82 => "8.2",
            PhpVersion::Php73Dbg => "7.3-dbg",
            PhpVersion::Php74Dbg => "7.4-dbg",
            PhpVersion::Php80Dbg => "8.0-dbg",
            PhpVersion::Php81Dbg => "8.1-dbg",
            PhpVersion::Php82Dbg => "8.2-dbg",
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
                memory: Some(PHP_MEMORY_LIMIT),
                nano_cpus: Some(2_000_000_000),
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

        let id = docker
            .create_container(options, config)
            .await
            .into_diagnostic()?
            .id;

        if let Err(e) = docker
            .start_container::<String>(&id, None)
            .await
            .into_diagnostic()
        {
            docker.remove_container(&id, None).await.ok();
            return Err(e);
        }

        if let Err(e) = docker
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
            .await
            .into_diagnostic()
        {
            docker.remove_container(&id, None).await.ok();
            return Err(e);
        }

        Ok(id)
    }

    pub async fn wait_for_start(&self, ip: Option<IpAddr>) -> Result<()> {
        let client = Client::new();
        let url = Url::parse(&format!(
            "http://{}/status.php",
            ip.ok_or(Report::msg("Container not running"))?
        ))
        .into_diagnostic()?;
        timeout(Duration::from_secs(15), async {
            while !client.get(url.clone()).send().await.is_ok() {
                sleep(Duration::from_millis(100)).await
            }
        })
        .await
        .into_diagnostic()
        .wrap_err("Timeout after 15 seconds")
    }
}

impl Default for PhpVersion {
    fn default() -> Self {
        PhpVersion::Php81
    }
}
