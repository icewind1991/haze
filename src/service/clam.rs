use crate::cloud::CloudOptions;
use crate::config::HazeConfig;
use crate::exec::exec;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, WrapErr};
use tokio::fs::write;

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
        _options: &CloudOptions,
    ) -> Result<Vec<String>> {
        let image = "ghcr.io/icewind1991/icap-clamav-service-tls";
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ClamIcapTls;

#[async_trait::async_trait]
impl ServiceTrait for ClamIcapTls {
    fn name(&self) -> &str {
        "clamav-icap-tls"
    }

    fn env(&self) -> &[&str] {
        &[
            "ICAP_HOST=clamav-icap-tls",
            "ICAP_PORT=1345",
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
        _options: &CloudOptions,
    ) -> Result<Vec<String>> {
        let image = "ghcr.io/icewind1991/icap-clamav-service-tls";
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
        Some(format!("{}-{}", cloud_id, self.name()))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_antivirus"]
    }

    async fn post_setup(
        &self,
        docker: &Docker,
        cloud_id: &str,
        config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let mut cert = Vec::new();
        exec(
            docker,
            self.container_name(cloud_id).unwrap(),
            "root",
            vec!["cat", "/local/cert.pem"],
            Vec::<String>::new(),
            Some(&mut cert),
        )
        .await
        .wrap_err("Failed to get icap certificate")?;

        let cert_path = config.work_dir.join(cloud_id).join("data/icap-cert.pem");
        write(cert_path, cert)
            .await
            .into_diagnostic()
            .wrap_err("Failed to write icap certificate")?;

        Ok(vec![
            "occ config:app:set files_antivirus av_mode --value=icap".into(),
            "occ config:app:set files_antivirus av_icap_tls --value=1".into(),
            "occ config:app:set files_antivirus av_host --value=clamav-icap-tls".into(),
            "occ config:app:set files_antivirus av_port --value=1345".into(),
            "occ config:app:set files_antivirus av_icap_request_service --value=avscan".into(),
            "occ config:app:set files_antivirus av_icap_response_header --value=X-Infection-Found"
                .into(),
            "occ security:certificates:import data/icap-cert.pem".into(),
        ])
    }
}
