use crate::config::HazeConfig;
use crate::exec::exec;
use crate::image::{image_exists, pull_image};
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{bail, IntoDiagnostic};
use std::io::Stdout;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Kaspersky;

#[async_trait::async_trait]
impl ServiceTrait for Kaspersky {
    fn name(&self) -> &str {
        "kaspersky"
    }

    fn env(&self) -> &[&str] {
        &["KASPERSKY_HOST=kaspersky", "KASPERSKY_PORT=80"]
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
            bail!("You need to manually create the 'kaspersky' image");
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
        let exit = exec(
            docker,
            self.container_name(cloud_id),
            "root",
            vec!["curl", "localhost/licenseinfo"],
            vec![],
            Option::<Stdout>::None,
        )
        .await?;
        Ok(exit.to_result().is_ok())
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-kaspersky", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_antivirus"]
    }

    async fn post_setup(&self, _docker: &Docker, _cloud_id: &str) -> Result<Vec<String>> {
        Ok(vec![
            "occ config:app:set files_antivirus av_mode --value=kaspersky".into(),
            "occ config:app:set files_antivirus av_host --value=kaspersky".into(),
            "occ config:app:set files_antivirus av_port --value=80".into(),
        ])
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KasperskyIcap;

#[async_trait::async_trait]
impl ServiceTrait for KasperskyIcap {
    fn name(&self) -> &str {
        "kaspersky-icap"
    }

    fn env(&self) -> &[&str] {
        &[
            "ICAP_HOST=kaspersky-icap",
            "ICAP_PORT=1344",
            "ICAP_REQUEST=req",
            "ICAP_HEADER=X-Virus-ID",
        ]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<String> {
        let image = "kaspersky-icap";
        if !image_exists(docker, image).await {
            bail!("You need to manually create the 'kaspersky-icap' image");
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

    // async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
    //     let exit = exec(
    //         docker,
    //         self.container_name(cloud_id),
    //         "root",
    //         vec!["curl", "localhost/licenseinfo"],
    //         vec![],
    //         Option::<Stdout>::None,
    //     )
    //     .await?;
    //     Ok(exit.to_result().is_ok())
    // }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-kaspersky-icap", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_antivirus"]
    }

    async fn post_setup(&self, _docker: &Docker, _cloud_id: &str) -> Result<Vec<String>> {
        Ok(vec![
            "occ config:app:set files_antivirus av_mode --value=icap".into(),
            "occ config:app:set files_antivirus av_host --value=kaspersky-icap".into(),
            "occ config:app:set files_antivirus av_port --value=1344".into(),
            "occ config:app:set files_antivirus av_icap_request_service --value=req".into(),
            "occ config:app:set files_antivirus av_icap_response_header --value=X-Infection-Found"
                .into(),
        ])
    }
}
