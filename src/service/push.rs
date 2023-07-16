use crate::config::HazeConfig;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, Result};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NotifyPush;

#[async_trait::async_trait]
impl ServiceTrait for NotifyPush {
    fn name(&self) -> &str {
        "push"
    }

    fn env(&self) -> &[&str] {
        &[]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        config: &HazeConfig,
    ) -> Result<Option<String>> {
        let image = "icewind1991/notify_push";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id).unwrap(),
            ..CreateContainerOptions::default()
        });
        let config = Config {
            image: Some(image),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                binds: Some(vec![
                    format!("{}/config:/config:ro", config.work_dir.join(cloud_id)),
                    format!("{}/data:/var/www/html/data", config.work_dir.join(cloud_id)),
                ]),
                ..Default::default()
            }),
            env: Some(vec![
                "NEXTCLOUD_URL=http://cloud/",
                "LOG=debug",
                "REDIS_URL=redis://cloud/",
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
            cmd: Some(vec!["/notify_push", "/config/config.php"]),
            ..Default::default()
        };
        let id = docker
            .create_container(options, config)
            .await
            .into_diagnostic()?
            .id;
        Ok(Some(id))
    }

    fn container_name(&self, cloud_id: &str) -> Option<String> {
        Some(format!("{}-push", cloud_id))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["notify_push"]
    }

    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn post_setup(
        &self,
        docker: &Docker,
        cloud_id: &str,
        config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let ip = self.get_ip(docker, cloud_id).await?.unwrap();
        let addr = config
            .proxy
            .addr_with_port(&self.container_name(cloud_id).unwrap(), ip, 7867);
        Ok(vec![
            format!("occ config:system:set trusted_proxies 1 --value {}", ip),
            format!("occ notify_push:setup {}", addr),
        ])
    }
}
