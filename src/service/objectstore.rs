use crate::config::HazeConfig;
use crate::exec::exec;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{ContainerState, EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::IntoDiagnostic;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectStore {
    S3,
    S3mb,
    Azure,
}

impl ObjectStore {
    fn image(&self) -> &str {
        match self {
            ObjectStore::S3 => "localstack/localstack:0.14.3",
            ObjectStore::S3mb => "localstack/localstack:0.14.3",
            ObjectStore::Azure => "arafato/azurite:2.6.5",
        }
    }

    fn self_env(&self) -> Vec<&str> {
        match self {
            ObjectStore::S3 => vec!["DEBUG=1", "SERVICES=s3"],
            ObjectStore::S3mb => vec!["DEBUG=1", "SERVICES=s3"],
            ObjectStore::Azure => vec![],
        }
    }
    fn host_name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
            ObjectStore::S3mb => "s3",
            ObjectStore::Azure => "azure",
        }
    }
}

#[async_trait::async_trait]
impl ServiceTrait for ObjectStore {
    fn name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
            ObjectStore::S3mb => "s3mb",
            ObjectStore::Azure => "azure",
        }
    }

    fn env(&self) -> &[&str] {
        match self {
            ObjectStore::S3 => &["S3=1"],
            ObjectStore::S3mb => &["S3MB=1"],
            ObjectStore::Azure => &["AZURE=1"],
        }
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<String> {
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
                        aliases: Some(vec![self.host_name().to_string()]),
                        ..Default::default()
                    }
                },
            }),
            ..Default::default()
        };
        let id = docker
            .create_container(options, config)
            .await
            .into_diagnostic()?
            .id;
        docker
            .start_container::<String>(&id, None)
            .await
            .into_diagnostic()?;
        Ok(id)
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        match self {
            ObjectStore::S3 | ObjectStore::S3mb => {
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
                let output = String::from_utf8(output).into_diagnostic()?;
                Ok(
                    output.contains(r#""s3": "running""#)
                        || output.contains(r#""s3": "available""#),
                )
            }
            _ => {
                let info = docker
                    .inspect_container(&self.container_name(cloud_id), None)
                    .await
                    .into_diagnostic()?;
                Ok(matches!(
                    info.state,
                    Some(ContainerState {
                        running: Some(true),
                        ..
                    })
                ))
            }
        }
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-object", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_external"]
    }
}
