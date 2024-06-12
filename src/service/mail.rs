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
pub struct Mail;

#[async_trait::async_trait]
impl ServiceTrait for Mail {
    fn name(&self) -> &str {
        "mail"
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<Option<String>> {
        let image = "rnwood/smtp4dev";
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
        Some(format!("{}-mail", cloud_id))
    }

    // no need to wait for mail, as it won't be used until the user logs in
    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn post_setup(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        Ok(vec![
            "occ config:system:set mail_smtpmode --value smtp".into(),
            "occ config:system:set mail_sendmailmode --value smtp".into(),
            "occ config:system:set mail_domain --value haze".into(),
            "occ config:system:set mail_smtphost --value mail".into(),
            "occ config:system:set mail_smtpport --value 25".into(),
            "occ user:setting admin settings email admin@haze".into(),
        ])
    }
}
