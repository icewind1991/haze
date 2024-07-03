use crate::config::HazeConfig;
use crate::exec::exec;
use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::IntoDiagnostic;
use std::io::Stdout;
use std::net::Ipv4Addr;
use tokio::spawn;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Oc;

#[async_trait::async_trait]
impl ServiceTrait for Oc {
    fn name(&self) -> &str {
        "oc"
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let image = "owncloud/server:10.12.2";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id).unwrap(),
            ..CreateContainerOptions::default()
        });
        let addr = config.proxy.addr(
            &self.container_name(cloud_id).unwrap(),
            Ipv4Addr::UNSPECIFIED.into(),
        );
        let domain = addr.split_once("://").unwrap().1;
        let env_trusted_domain = format!("OWNCLOUD_TRUSTED_DOMAINS={domain}");
        let env_domain = format!("OWNCLOUD_DOMAIN={domain}");
        let config = Config {
            image: Some(image),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                ..Default::default()
            }),
            env: Some(vec![&env_trusted_domain, &env_domain]),
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
        Some(format!("{}-oc", cloud_id))
    }

    // no need to wait for oc
    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn post_setup(
        &self,
        docker: &Docker,
        cloud_id: &str,
        config: &HazeConfig,
    ) -> Result<Vec<String>> {
        if let Some(ip) = self.get_ip(docker, cloud_id).await? {
            let container = self.container_name(cloud_id).unwrap();
            let addr = config.proxy.addr(&container, ip);
            println!("OC running on {addr}");
            let docker = docker.clone();
            spawn(async move {
                let (protocol, domain) = addr.split_once("://").unwrap();
                simple_exec(
                    &docker,
                    &container,
                    vec![
                        vec![
                            "occ",
                            "config:system:set",
                            "overwrite.cli.url",
                            "--value",
                            &addr,
                        ],
                        vec![
                            "occ",
                            "config:system:set",
                            "overwritehost",
                            "--value",
                            domain,
                        ],
                        vec![
                            "occ",
                            "config:system:set",
                            "overwriteprotocol",
                            "--value",
                            protocol,
                        ],
                        vec!["apt", "update"],
                        vec!["apt-get", "install", "-y", "neovim", "ripgrep"],
                    ],
                )
                .await
                .ok();
                exec(
                    &docker,
                    &container,
                    "root",
                    vec!["occ", "user:add", "test", "--password-from-env"],
                    vec!["OC_PASS=test"],
                    None::<Stdout>,
                )
                .await
                .ok();
            });
        }
        Ok(Vec::new())
    }

    fn proxy_port(&self) -> u16 {
        8080
    }
}

async fn simple_exec(docker: &Docker, container: &str, cmds: Vec<Vec<&str>>) -> Result<()> {
    for cmd in cmds {
        exec(
            docker,
            container,
            "root",
            cmd,
            Vec::<String>::new(),
            None::<Stdout>,
        )
        .await?;
    }
    Ok(())
}
