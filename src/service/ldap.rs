use crate::image::pull_image;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{ContainerState, EndpointSettings, HostConfig};
use bollard::Docker;
use color_eyre::Report;
use maplit::hashmap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LDAP;

#[async_trait::async_trait]
impl ServiceTrait for LDAP {
    fn name(&self) -> &str {
        "ldap"
    }

    fn env(&self) -> &[&str] {
        &["LDAP=1"]
    }

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        let image = "icewind1991/haze-ldap";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
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
        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;
        Ok(id)
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-ldap", cloud_id)
    }

    async fn start_message(&self, _docker: &Docker, _cloud_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["user_ldap"]
    }

    async fn post_setup(&self, _docker: &Docker, _cloud_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LDAPAdmin;

#[async_trait::async_trait]
impl ServiceTrait for LDAPAdmin {
    fn name(&self) -> &str {
        "ldap-admin"
    }

    fn env(&self) -> &[&str] {
        &[]
    }

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        let image = "osixia/phpldapadmin";
        pull_image(docker, image).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
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
        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;
        Ok(id)
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-ldap-admin", cloud_id)
    }

    async fn start_message(&self, docker: &Docker, cloud_id: &str) -> Result<Option<String>> {
        let info = docker
            .inspect_container(&self.container_name(cloud_id), None)
            .await?;
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

    async fn post_setup(&self, _docker: &Docker, _cloud_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn apps(&self) -> &'static [&'static str] {
        &[]
    }
}
