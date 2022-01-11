mod kaspersky;
mod ldap;
mod objectstore;
mod onlyoffice;
mod push;
mod smb;

use crate::config::HazeConfig;
use crate::service::kaspersky::Kaspersky;
pub use crate::service::ldap::{LDAPAdmin, LDAP};
pub use crate::service::objectstore::ObjectStore;
pub use crate::service::onlyoffice::OnlyOffice;
pub use crate::service::push::NotifyPush;
use crate::service::smb::Smb;
use bollard::models::ContainerState;
use bollard::Docker;
use enum_dispatch::enum_dispatch;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[async_trait::async_trait]
#[enum_dispatch(Service)]
pub trait ServiceTrait {
    fn name(&self) -> &str;

    fn env(&self) -> &[&str] {
        &[]
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
    ) -> Result<String>;

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
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

    fn container_name(&self, cloud_id: &str) -> String;

    async fn start_message(&self, _docker: &Docker, _cloud_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn apps(&self) -> &'static [&'static str] {
        &[]
    }

    async fn post_setup(&self, _docker: &Docker, _cloud_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[enum_dispatch]
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Service {
    ObjectStore(ObjectStore),
    LDAP(LDAP),
    LDAPAdmin(LDAPAdmin),
    OnlyOffice(OnlyOffice),
    Push(NotifyPush),
    Smb(Smb),
    Kaspersky(Kaspersky),
}

impl Service {
    pub fn from_type(ty: &str) -> Option<&'static [Self]> {
        match ty {
            "s3" => Some(&[Service::ObjectStore(ObjectStore::S3)]),
            "s3mb" => Some(&[Service::ObjectStore(ObjectStore::S3mb)]),
            "ldap" => Some(&[Service::LDAP(LDAP), Service::LDAPAdmin(LDAPAdmin)]),
            "onlyoffice" => Some(&[Service::OnlyOffice(OnlyOffice)]),
            "push" => Some(&[Service::Push(NotifyPush)]),
            "smb" => Some(&[Service::Smb(Smb)]),
            "kaspersky" => Some(&[Service::Kaspersky(Kaspersky)]),
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
        .into_diagnostic()
        .wrap_err("Timeout after 30 seconds")?
    }
}
