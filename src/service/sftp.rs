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
pub struct Sftp;

#[async_trait::async_trait]
impl ServiceTrait for Sftp {
    fn name(&self) -> &str {
        "sftp"
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
        _options: &CloudOptions,
    ) -> Result<Vec<String>> {
        let image = "atmoz/sftp:alpine";
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
            cmd: Some(vec!["test:test:::data"]),
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
        Some(format!("{}-sftp", cloud_id))
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
        Ok(vec![
            "occ files_external:create sftp sftp password::password".into(),
            "occ files_external:config 1 host sftp".into(),
            "occ files_external:config 1 user test".into(),
            "occ files_external:config 1 root data".into(),
            "occ files_external:config 1 password test".into(),
        ])
    }
}
