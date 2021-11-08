use crate::config::HazeConfig;
use crate::image::{image_exists, pull_image};
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use color_eyre::eyre::eyre;
use maplit::hashmap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Kaspersky;

#[async_trait::async_trait]
impl ServiceTrait for Kaspersky {
    fn name(&self) -> &str {
        "kaspersky"
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
        let image = "kaspersky";
        if !image_exists(docker, image).await {
            eyre!("You need to manually create the 'kaspersky' image");
        }
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

    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-kaspersky", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_antivirus"]
    }
}
