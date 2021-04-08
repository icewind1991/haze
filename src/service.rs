use crate::exec::exec;
use crate::image::pull_image;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};
use maplit::hashmap;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Service {
    ObjectStore(ObjectStore),
}

impl Service {
    pub fn name(&self) -> &str {
        match self {
            Service::ObjectStore(store) => store.name(),
        }
    }

    pub fn env(&self) -> &[&str] {
        match self {
            Service::ObjectStore(store) => store.env(),
        }
    }

    pub async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        match self {
            Service::ObjectStore(store) => store.spawn(docker, cloud_id, network).await,
        }
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        match self {
            Service::ObjectStore(store) => store.is_healthy(docker, cloud_id).await,
        }
    }

    pub fn from_type(ty: &str) -> Option<Self> {
        match ty {
            "s3" => Some(Service::ObjectStore(ObjectStore::S3)),
            _ => None,
        }
    }

    pub async fn wait_for_start(&self, docker: &Docker, cloud_id: &str) -> Result<()> {
        timeout(Duration::from_secs(30), async {
            while !self.is_healthy(docker, cloud_id).await? {
                sleep(Duration::from_millis(100)).await
            }
            Ok(())
        })
        .await
        .wrap_err("Timeout after 30 seconds")?
    }

    pub fn container_name(&self, cloud_id: &str) -> String {
        match self {
            Service::ObjectStore(store) => store.container_name(cloud_id),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectStore {
    S3,
}

impl ObjectStore {
    fn image(&self) -> &str {
        match self {
            ObjectStore::S3 => "localstack/localstack:0.12.7",
        }
    }

    fn name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
        }
    }

    fn self_env(&self) -> Vec<&str> {
        match self {
            ObjectStore::S3 => vec!["DEBUG=1", "SERVICES=s3"],
        }
    }

    fn env(&self) -> &[&str] {
        match self {
            ObjectStore::S3 => &["S3=1"],
        }
    }

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: format!("{}-object", cloud_id),
        });
        let config = Config {
            image: Some(self.image()),
            env: Some(self.self_env()),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                ..Default::default()
            }),
            labels: Some(hashmap! {
                "haze-type" => self.name(),
                "haze-cloud-id" => cloud_id
            }),
            networking_config: Some(NetworkingConfig {
                endpoints_config: hashmap! {
                    network => EndpointSettings {
                        aliases: Some(vec![self.name().to_string()]),
                        ..Default::default()
                    }
                },
            }),
            ..Default::default()
        };
        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;
        Ok(id)
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        let mut output = Vec::new();
        exec(
            docker,
            format!("{}-object", cloud_id),
            "root",
            vec!["curl", "localhost:4566/health"],
            vec![],
            Some(&mut output),
        )
        .await?;
        let output = String::from_utf8(output)?;
        Ok(output.contains(r#""s3": "running""#))
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-object", cloud_id)
    }
}
