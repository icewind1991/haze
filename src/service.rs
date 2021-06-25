use crate::exec::exec;
use crate::image::pull_image;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{ContainerState, EndpointSettings, HostConfig};
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Report, Result};
use maplit::hashmap;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Service {
    ObjectStore(ObjectStore),
    Ldap(LDAP),
    LdapAdmin(LDAPAdmin),
}

impl Service {
    pub fn name(&self) -> &str {
        match self {
            Service::ObjectStore(store) => store.name(),
            Service::Ldap(ldap) => ldap.name(),
            Service::LdapAdmin(ldap_admin) => ldap_admin.name(),
        }
    }

    pub fn env(&self) -> &[&str] {
        match self {
            Service::ObjectStore(store) => store.env(),
            Service::Ldap(ldap) => ldap.env(),
            Service::LdapAdmin(ldap_admin) => ldap_admin.env(),
        }
    }

    pub async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        match self {
            Service::ObjectStore(store) => store.spawn(docker, cloud_id, network).await,
            Service::Ldap(ldap) => ldap.spawn(docker, cloud_id, network).await,
            Service::LdapAdmin(ldap_admin) => ldap_admin.spawn(docker, cloud_id, network).await,
        }
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        match self {
            Service::ObjectStore(store) => store.is_healthy(docker, cloud_id).await,
            Service::Ldap(ldap) => ldap.is_healthy(docker, cloud_id).await,
            Service::LdapAdmin(ldap_admin) => ldap_admin.is_healthy(docker, cloud_id).await,
        }
    }

    pub fn from_type(ty: &str) -> Option<&'static [Self]> {
        match ty {
            "s3" => Some(&[Service::ObjectStore(ObjectStore::S3)]),
            "ldap" => Some(&[Service::Ldap(LDAP), Service::LdapAdmin(LDAPAdmin)]),
            _ => None,
        }
    }

    pub async fn wait_for_start(&self, docker: &Docker, cloud_id: &str) -> Result<()> {
        timeout(Duration::from_secs(30), async {
            while !self.is_healthy(docker, cloud_id).await? {
                sleep(Duration::from_millis(100)).await
            }
            Ok(())
        })
        .await
        .wrap_err("Timeout after 30 seconds")?
    }

    pub fn container_name(&self, cloud_id: &str) -> String {
        match self {
            Service::ObjectStore(store) => store.container_name(cloud_id),
            Service::Ldap(ldap) => ldap.container_name(cloud_id),
            Service::LdapAdmin(ldap_admin) => ldap_admin.container_name(cloud_id),
        }
    }

    pub async fn start_message(&self, docker: &Docker, cloud_id: &str) -> Result<Option<String>> {
        match self {
            Service::ObjectStore(store) => store.start_message(docker, cloud_id).await,
            Service::Ldap(ldap) => ldap.start_message(docker, cloud_id).await,
            Service::LdapAdmin(ldap_admin) => ldap_admin.start_message(docker, cloud_id).await,
        }
    }

    pub fn apps(&self) -> &'static [&'static str] {
        match self {
            Service::ObjectStore(store) => store.apps(),
            Service::Ldap(ldap) => ldap.apps(),
            Service::LdapAdmin(ldap_admin) => ldap_admin.apps(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectStore {
    S3,
}

impl ObjectStore {
    fn image(&self) -> &str {
        match self {
            ObjectStore::S3 => "localstack/localstack:0.12.7",
        }
    }

    fn name(&self) -> &str {
        match self {
            ObjectStore::S3 => "s3",
        }
    }

    fn self_env(&self) -> Vec<&str> {
        match self {
            ObjectStore::S3 => vec!["DEBUG=1", "SERVICES=s3"],
        }
    }

    fn env(&self) -> &[&str] {
        match self {
            ObjectStore::S3 => &["S3=1"],
        }
    }

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: format!("{}-object", cloud_id),
        });
        let config = Config {
            image: Some(self.image()),
            env: Some(self.self_env()),
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
        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;
        Ok(id)
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        let mut output = Vec::new();
        exec(
            docker,
            format!("{}-object", cloud_id),
            "root",
            vec!["curl", "localhost:4566/health"],
            vec![],
            Some(&mut output),
        )
        .await?;
        let output = String::from_utf8(output)?;
        Ok(output.contains(r#""s3": "running""#))
    }

    fn container_name(&self, cloud_id: &str) -> String {
        format!("{}-object", cloud_id)
    }

    async fn start_message(&self, _docker: &Docker, _cloud_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn apps(&self) -> &'static [&'static str] {
        &["files_external"]
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LDAP;

impl LDAP {
    fn image(&self) -> &str {
        "icewind1991/haze-ldap"
    }

    fn name(&self) -> &str {
        "ldap"
    }

    fn self_env(&self) -> Vec<&str> {
        vec!["LDAP_ADMIN_PASSWORD=haze"]
    }

    fn env(&self) -> &[&str] {
        &["LDAP=1"]
    }

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
        });
        let config = Config {
            image: Some(self.image()),
            env: Some(self.self_env()),
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

    async fn is_healthy(&self, _docker: &Docker, _cloud_id: &str) -> Result<bool> {
        Ok(true)
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
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LDAPAdmin;

impl LDAPAdmin {
    fn image(&self) -> &str {
        "osixia/phpldapadmin"
    }

    fn name(&self) -> &str {
        "ldap-admin"
    }

    fn self_env(&self) -> Vec<&str> {
        vec!["PHPLDAPADMIN_LDAP_HOSTS=ldap"]
    }

    fn env(&self) -> &[&str] {
        &[]
    }

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String> {
        pull_image(docker, self.image()).await?;
        let options = Some(CreateContainerOptions {
            name: self.container_name(cloud_id),
        });
        let config = Config {
            image: Some(self.image()),
            env: Some(self.self_env()),
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

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        let info = docker
            .inspect_container(&self.container_name(cloud_id), None)
            .await?;
        Ok(matches!(
            info.state,
            Some(ContainerState {
                running: Some(true),
                ..
            })
        ))
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

    fn apps(&self) -> &'static [&'static str] {
        &[]
    }
}
