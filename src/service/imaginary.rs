use crate::cloud::CloudOptions;
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
pub struct Imaginary;

#[async_trait::async_trait]
impl ServiceTrait for Imaginary {
    fn name(&self) -> &str {
        "imaginary"
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
        _options: &CloudOptions,
    ) -> Result<Vec<String>> {
        let image = "nextcloud/aio-imaginary:latest";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id).unwrap(),
            ..CreateContainerOptions::default()
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
        let id = docker
            .create_container(options, config)
            .await
            .into_diagnostic()?
            .id;
        docker
            .start_container::<String>(&id, None)
            .await
            .into_diagnostic()?;
        Ok(vec![id])
    }

    fn container_name(&self, cloud_id: &str) -> Option<String> {
        Some(format!("{}-imaginary", cloud_id))
    }

    async fn post_setup(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        Ok(vec![
            "occ config:system:set enabledPreviewProviders 0 --value='OC\\Preview\\Imaginary'"
                .into(),
            "occ config:system:set preview_imaginary_url --value='http://imaginary:9000'".into(),
        ])
    }
}
