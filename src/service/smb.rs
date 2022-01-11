use crate::config::HazeConfig;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::IntoDiagnostic;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Smb;

#[async_trait::async_trait]
impl ServiceTrait for Smb {
    fn name(&self) -> &str {
        "smb"
    }

    fn env(&self) -> &[&str] {
        &[]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<String> {
        let image = "servercontainers/samba";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
        });
        let config = Config {
            image: Some(image),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                ..Default::default()
            }),
            env: Some(vec![
                "ACCOUNT_test=test",
                "UID_test=1000",
                "SAMBA_VOLUME_CONFIG_test=[test]; path=/tmp; valid users = test; guest ok = no; read only = no; browseable = yes",
            ]),
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

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-smb", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_external"]
    }

    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
    }
}
