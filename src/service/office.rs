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
        config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let image = "collabora/code";
        pull_image(docker, image).await?;
        let container_id = self.container_name(cloud_id).unwrap();
        let options = Some(CreateContainerOptions {
            name: container_id.clone(),
            ..CreateContainerOptions::default()
        });
        let mut env = vec!["extra_params=--o:ssl.enable=false --o:ssl.termination=true"];

        let clean_id = container_id.strip_prefix("haze-").unwrap_or(&container_id);
        let server_name_opt = match (&config.proxy.address, config.proxy.https) {
            (public, true) if !public.is_empty() => {
                format!("server_name={clean_id}.{public}")
            }
            (public, false) if !public.is_empty() => {
                format!("server_name={clean_id}.{public}")
            }
            _ => "".to_string(),
        };

        if !server_name_opt.is_empty() {
            env.push(&server_name_opt);
        }

        let config = Config {
            image: Some(image),
            env: Some(env),
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
        Some(format!("{}-office", cloud_id))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["richdocuments"]
    }

    async fn post_setup(
        &self,
        docker: &Docker,
        cloud_id: &str,
        config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let container = &self.container_name(cloud_id).unwrap();
        let info = docker
            .inspect_container(container, None)
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
                .parse()
                .into_diagnostic()?
        } else {
            return Err(Report::msg("office not started"));
        };
        Ok(vec![
            format!(
                r#"occ config:app:set richdocuments wopi_url --value="http://{}:9980""#,
                ip
            ),
            format!(
                r#"occ config:app:set richdocuments public_wopi_url --value="{}""#,
                config.proxy.addr_with_port(container, ip, 9980)
            ),
            format!(
                r#"occ config:app:set richdocuments wopi_root --value="http://{}""#,
                cloud_id
            ),
        ])
    }

    fn proxy_port(&self) -> u16 {
        9980
    }
}
