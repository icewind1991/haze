mod clam;
mod dav;
mod kaspersky;
mod ldap;
mod objectstore;
mod office;
mod onlyoffice;
mod push;
mod sftp;
mod smb;

use crate::config::HazeConfig;
pub use crate::service::clam::ClamIcap;
use crate::service::dav::Dav;
use crate::service::kaspersky::{Kaspersky, KasperskyIcap};
pub use crate::service::ldap::{Ldap, LdapAdmin};
pub use crate::service::objectstore::ObjectStore;
pub use crate::service::office::Office;
pub use crate::service::onlyoffice::OnlyOffice;
pub use crate::service::push::NotifyPush;
use crate::service::sftp::Sftp;
use crate::service::smb::Smb;
use bollard::models::ContainerState;
use bollard::Docker;
use enum_dispatch::enum_dispatch;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use std::net::IpAddr;
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

    async fn post_setup(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn is_running(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
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

    async fn wait_for_running(&self, docker: &Docker, cloud_id: &str) -> Result<()> {
        timeout(Duration::from_secs(30), async {
            while !self.is_running(docker, cloud_id).await? {
                sleep(Duration::from_millis(100)).await
            }
            Ok(())
        })
        .await
        .into_diagnostic()
        .wrap_err("Timeout after 30 seconds")?
    }

    async fn get_ip(&self, docker: &Docker, cloud_id: &str) -> Result<IpAddr> {
        docker
            .start_container::<String>(&self.container_name(cloud_id), None)
            .await
            .into_diagnostic()?;
        self.wait_for_running(docker, cloud_id).await?;

        sleep(Duration::from_millis(100)).await;

        let info = docker
            .inspect_container(&self.container_name(cloud_id), None)
            .await
            .into_diagnostic()?;
        if matches!(
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
                .into_diagnostic()
                .wrap_err("Invalid ip address")
        } else {
            Err(Report::msg("service not started"))
        }
    }
}

#[enum_dispatch]
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Service {
    ObjectStore(ObjectStore),
    Ldap(Ldap),
    LdapAdmin(LdapAdmin),
    OnlyOffice(OnlyOffice),
    Office(Office),
    Push(NotifyPush),
    Smb(Smb),
    Dav(Dav),
    Sftp(Sftp),
    Kaspersky(Kaspersky),
    KasperskyIcap(KasperskyIcap),
    ClamIcap(ClamIcap),
}

impl Service {
    pub fn from_type(ty: &str) -> Option<&'static [Self]> {
        match ty {
            "s3" => Some(&[Service::ObjectStore(ObjectStore::S3)]),
            "s3m" => Some(&[Service::ObjectStore(ObjectStore::S3m)]),
            "s3mb" => Some(&[Service::ObjectStore(ObjectStore::S3mb)]),
            "azure" => Some(&[Service::ObjectStore(ObjectStore::Azure)]),
            "ldap" => Some(&[Service::Ldap(Ldap), Service::LdapAdmin(LdapAdmin)]),
            "onlyoffice" => Some(&[Service::OnlyOffice(OnlyOffice)]),
            "office" => Some(&[Service::Office(Office)]),
            "push" => Some(&[Service::Push(NotifyPush)]),
            "smb" => Some(&[Service::Smb(Smb)]),
            "dav" => Some(&[Service::Dav(Dav)]),
            "sftp" => Some(&[Service::Sftp(Sftp)]),
            "kaspersky" => Some(&[Service::Kaspersky(Kaspersky)]),
            "kaspersky-icap" => Some(&[Service::KasperskyIcap(KasperskyIcap)]),
            "clamav-icap" => Some(&[Service::ClamIcap(ClamIcap)]),
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
