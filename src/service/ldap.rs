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
pub struct Ldap;

#[async_trait::async_trait]
impl ServiceTrait for Ldap {
    fn name(&self) -> &str {
        "ldap"
    }

    fn env(&self) -> &[&str] {
        &["LDAP=1"]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let image = "icewind1991/haze-ldap";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id).unwrap(),
            ..CreateContainerOptions::default()
        });
        let config = Config {
            image: Some(image),
            env: Some(vec!["LDAP_ADMIN_PASSWORD=haze"]),
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
            cmd: Some(vec!["--copy-service"]),
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
        Some(format!("{}-ldap", cloud_id))
    }

    fn apps(&self) -> &'static [&'static str] {
        &["user_ldap"]
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LdapAdmin;

#[async_trait::async_trait]
impl ServiceTrait for LdapAdmin {
    fn name(&self) -> &str {
        "ldap-admin"
    }

    fn env(&self) -> &[&str] {
        &[]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        let image = "osixia/phpldapadmin";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id).unwrap(),
            ..CreateContainerOptions::default()
        });
        let config = Config {
            image: Some(image),
            env: Some(vec!["PHPLDAPADMIN_LDAP_HOSTS=ldap"]),
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
            cmd: Some(vec!["--copy-service"]),
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
        Some(format!("{}-ldap-admin", cloud_id))
    }

    async fn start_message(&self, docker: &Docker, cloud_id: &str) -> Result<Option<String>> {
        let info = docker
            .inspect_container(&self.container_name(cloud_id).unwrap(), None)
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
            return Err(Report::msg("ldap admin not started"));
        };
        Ok(Some(format!(
            "Ldap admin running at: https://{} with 'cn=admin,dc=example,dc=org' and password 'haze'",
            ip
        )))
    }
}
