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
    S3m,
    S3mb,
    Azure,
}

impl ObjectStore {
    fn image(&self) -> &str {
        match self {
            ObjectStore::S3 | ObjectStore::S3m | ObjectStore::S3mb => {
                "minio/minio:RELEASE.2023-01-20T02-05-44Z.hotfix.b9b60d73d"
            }
            ObjectStore::Azure => "arafato/azurite:2.6.5",
        }
    }

    fn self_env(&self) -> Vec<&str> {
        match self {
            ObjectStore::S3 | ObjectStore::S3m | ObjectStore::S3mb => {
                vec!["MINIO_ACCESS_KEY=minio", "MINIO_SECRET_KEY=minio123"]
            }
            ObjectStore::Azure => vec![],
        }
    }

    fn host_name(&self) -> &str {
        match self {
            ObjectStore::S3 | ObjectStore::S3m | ObjectStore::S3mb => "s3",
            ObjectStore::Azure => "azure",
        }
    }

    fn args(&self) -> &[&str] {
        match self {
            ObjectStore::S3 | ObjectStore::S3m | ObjectStore::S3mb => &["server", "/data"],
            _ => &[],
        }
    }
}

#[async_trait::async_trait]
impl ServiceTrait for ObjectStore {
    fn name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
            ObjectStore::S3m => "s3m",
            ObjectStore::S3mb => "s3mb",
            ObjectStore::Azure => "azure",
        }
    }

    fn env(&self) -> &[&str] {
        match self {
            ObjectStore::S3 => &["S3=1"],
            ObjectStore::S3m => &["S3M=1"],
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
    ) -> Result<Option<String>> {
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: format!("{}-object", cloud_id),
            ..CreateContainerOptions::default()
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
            cmd: Some(self.args().into()),
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
        Ok(Some(id))
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        match self {
            ObjectStore::S3 | ObjectStore::S3mb => {
                let mut output = Vec::new();
                let exit = exec(
                    docker,
                    format!("{}-object", cloud_id),
                    "root",
                    vec!["curl", "localhost:9000/minio/health/ready"],
                    Vec::<String>::default(),
                    Some(&mut output),
                )
                .await?;
                Ok(exit.is_ok())
            }
            _ => {
                let info = docker
                    .inspect_container(&self.container_name(cloud_id).unwrap(), None)
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

    fn container_name(&self, cloud_id: &str) -> Option<String> {
        Some(format!("{}-object", cloud_id))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_external"]
    }

    async fn post_setup(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        if *self == ObjectStore::S3 {
            Ok(vec![
                "occ files_external:create s3 amazons3 amazons3::accesskey".into(),
                "occ files_external:config 1 bucket ext".into(),
                "occ files_external:config 1 hostname s3".into(),
                "occ files_external:config 1 port 9000".into(),
                "occ files_external:config 1 use_ssl false".into(),
                "occ files_external:config 1 use_path_style true".into(),
                "occ files_external:config 1 key minio".into(),
                "occ files_external:config 1 secret minio123".into(),
            ])
        } else {
            Ok(Vec::new())
        }
    }
}
