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
pub struct ClamIcap;

#[async_trait::async_trait]
impl ServiceTrait for ClamIcap {
    fn name(&self) -> &str {
        "clamav-icap"
    }

    fn env(&self) -> &[&str] {
        &[
            "ICAP_HOST=clamav-icap",
            "ICAP_PORT=1344",
            "ICAP_REQUEST=avscan",
            "ICAP_HEADER=X-Infection-Found",
        ]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<Option<String>> {
        let image = "deepdiver/icap-clamav-service";
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
        Some(format!("{}-clamav-icap", cloud_id))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_antivirus"]
    }

    async fn post_setup(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        Ok(vec![
            "occ config:app:set files_antivirus av_mode --value=icap".into(),
            "occ config:app:set files_antivirus av_host --value=clamav-icap".into(),
            "occ config:app:set files_antivirus av_port --value=1344".into(),
            "occ config:app:set files_antivirus av_icap_request_service --value=avscan".into(),
            "occ config:app:set files_antivirus av_icap_response_header --value=X-Infection-Found"
                .into(),
        ])
    }
}
