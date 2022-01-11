use crate::config::HazeConfig;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{ContainerState, EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use std::time::Duration;
use tokio::time::{sleep, timeout};

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
    ) -> Result<String> {
        let image = "icewind1991/notify_push";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
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
        Ok(id)
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-push", cloud_id)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["notify_push"]
    }

    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn post_setup(&self, docker: &Docker, cloud_id: &str) -> Result<Vec<String>> {
        docker
            .start_container::<String>(&self.container_name(cloud_id), None)
            .await
            .into_diagnostic()?;
        self.wait_for_push(docker, cloud_id).await?;

        sleep(Duration::from_millis(100)).await;

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
            return Err(Report::msg("notify_push not started"));
        };
        Ok(vec![
            format!("occ config:system:set trusted_proxies 1 --value {}", ip),
            format!("occ notify_push:setup http://{}:7867", ip),
        ])
    }
}

impl NotifyPush {
    async fn is_push_running(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        let info = docker
            .inspect_container(&self.container_name(cloud_id), None)
            .await
            .into_diagnostic()?;
        Ok(matches!(
            info.state,
            Some(ContainerState {
                running: Some(true),
                ..
            })
        ))
    }

    async fn wait_for_push(&self, docker: &Docker, cloud_id: &str) -> Result<()> {
        timeout(Duration::from_secs(30), async {
            while !self.is_push_running(docker, cloud_id).await? {
                sleep(Duration::from_millis(100)).await
            }
            Ok(())
        })
        .await
        .into_diagnostic()
        .wrap_err("Timeout after 30 seconds")?
    }
}
