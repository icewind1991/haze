use crate::config::HazeConfig;
use crate::exec::exec;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::IntoDiagnostic;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectStore {
    S3,
    S3mb,
}

impl ObjectStore {
    fn image(&self) -> &str {
        match self {
            ObjectStore::S3 => "localstack/localstack:0.14.3",
            ObjectStore::S3mb => "localstack/localstack:0.14.3",
        }
    }

    fn self_env(&self) -> Vec<&str> {
        match self {
            ObjectStore::S3 => vec!["DEBUG=1", "SERVICES=s3"],
            ObjectStore::S3mb => vec!["DEBUG=1", "SERVICES=s3"],
        }
    }
    fn host_name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
            ObjectStore::S3mb => "s3",
        }
    }
}

#[async_trait::async_trait]
impl ServiceTrait for ObjectStore {
    fn name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
            ObjectStore::S3mb => "s3mb",
        }
    }

    fn env(&self) -> &[&str] {
        match self {
            ObjectStore::S3 => &["S3=1"],
            ObjectStore::S3mb => &["S3MB=1"],
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
        Ok(output.contains(r#""s3": "running""#) || output.contains(r#""s3": "available""#))
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-object", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_external"]
    }
}
