use crate::config::HazeConfig;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{ContainerState, EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, Report};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OnlyOffice;

#[async_trait::async_trait]
impl ServiceTrait for OnlyOffice {
    fn name(&self) -> &str {
        "onlyoffice"
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
    ) -> Result<Option<String>> {
        let image = "onlyoffice/documentserver";
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
        Ok(Some(id))
    }

    fn container_name(&self, cloud_id: &str) -> Option<String> {
        Some(format!("{}-onlyoffice", cloud_id))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["onlyoffice"]
    }

    async fn post_setup(
        &self,
        docker: &Docker,
        cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let info = docker
            .inspect_container(&self.container_name(cloud_id).unwrap(), None)
            .await
            .into_diagnostic()?;
        let ip = if matches!(
            info.state,
            Some(ContainerState {
                running: Some(true),
                ..
            })
        ) {
            info.network_settings
                .unwrap()
                .networks
                .unwrap()
                .values()
                .next()
                .unwrap()
                .ip_address
                .clone()
                .unwrap()
        } else {
            return Err(Report::msg("onlyoffice not started"));
        };
        Ok(vec![format!(
            "occ config:app:set onlyoffice DocumentServerUrl --value http://{}/",
            ip
        )])
    }
}
