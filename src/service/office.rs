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
pub struct Office;

#[async_trait::async_trait]
impl ServiceTrait for Office {
    fn name(&self) -> &str {
        "office"
    }

    fn env(&self) -> &[&str] {
        &["COOL_HOST=office"]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<String> {
        let image = "collabora/code";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
        });
        let config = Config {
            image: Some(image),
            env: Some(vec!["extra_params=--o:ssl.enable=false"]),
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
        Ok(id)
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-office", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["richdocuments"]
    }

    async fn post_setup(&self, docker: &Docker, cloud_id: &str) -> Result<Vec<String>> {
        let info = docker
            .inspect_container(&self.container_name(cloud_id), None)
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
            return Err(Report::msg("office not started"));
        };
        Ok(vec![
            format!(
                r#"occ config:app:set richdocuments wopi_url --value="http://{}:9980""#,
                ip
            ),
            format!(
                r#"occ config:app:set richdocuments public_wopi_url --value="http://{}:9980""#,
                ip
            ),
            format!(
                r#"occ config:app:set richdocuments wopi_root --value="http://{}""#,
                cloud_id
            ),
        ])
    }
}
