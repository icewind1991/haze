mod ldap;
mod objectstore;
mod onlyoffice;

use crate::service::ldap::{LDAPAdmin, LDAP};
use crate::service::objectstore::ObjectStore;
use crate::service::onlyoffice::OnlyOffice;
use bollard::models::ContainerState;
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};
use enum_dispatch::enum_dispatch;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[async_trait::async_trait]
#[enum_dispatch(Service)]
pub trait ServiceTrait {
    fn name(&self) -> &str;

    fn env(&self) -> &[&str];

    async fn spawn(&self, docker: &Docker, cloud_id: &str, network: &str) -> Result<String>;

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

    fn container_name(&self, cloud_id: &str) -> String;

    async fn start_message(&self, docker: &Docker, cloud_id: &str) -> Result<Option<String>>;

    fn apps(&self) -> &'static [&'static str];

    async fn post_setup(&self, docker: &Docker, cloud_id: &str) -> Result<Vec<String>>;
}

#[enum_dispatch]
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Service {
    ObjectStore(ObjectStore),
    LDAP(LDAP),
    LDAPAdmin(LDAPAdmin),
    OnlyOffice(OnlyOffice),
}

impl Service {
    pub fn from_type(ty: &str) -> Option<&'static [Self]> {
        match ty {
            "s3" => Some(&[Service::ObjectStore(ObjectStore::S3)]),
            "ldap" => Some(&[Service::LDAP(LDAP), Service::LDAPAdmin(LDAPAdmin)]),
            "onlyoffice" => Some(&[Service::OnlyOffice(OnlyOffice)]),
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
}
