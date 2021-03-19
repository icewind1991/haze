use crate::config::HazeConfig;
use crate::database::Database;
use crate::exec::{exec, exec_tty};
use crate::mapping::default_mappings;
use crate::php::PhpVersion;
use crate::service::Service;
use bollard::container::{ListContainersOptions, RemoveContainerOptions};
use bollard::models::ContainerState;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::Utf8PathBuf;
use color_eyre::{eyre::WrapErr, Report, Result};
use futures_util::future::try_join_all;
use petname::petname;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::io::stdout;
use std::iter::Peekable;
use std::net::IpAddr;
use std::os::unix::fs::MetadataExt;
use std::str::FromStr;
use std::time::Duration;
use tokio::fs::remove_dir_all;
use tokio::time::sleep;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct CloudOptions {
    db: Database,
    php: PhpVersion,
    services: Vec<Service>,
}

impl CloudOptions {
    pub fn parse<I, S>(args: &mut Peekable<I>) -> Result<CloudOptions>
    where
        S: AsRef<str> + Into<String> + Display,
        I: Iterator<Item = S>,
    {
        let mut db = None;
        let mut php = None;
        let mut services = Vec::new();

        while let Some(option) = args.peek() {
            if let Ok(db_option) = Database::from_str(option.as_ref()) {
                db = Some(db_option);
                let _ = args.next();
            } else if let Ok(php_option) = PhpVersion::from_str(option.as_ref()) {
                php = Some(php_option);
                let _ = args.next();
            } else if let Some(service) = Service::from_type(option.as_ref()) {
                services.push(service);
                let _ = args.next();
            } else {
                break;
            }

            if db.is_some() && php.is_some() {
                break;
            }
        }

        Ok(CloudOptions {
            db: db.unwrap_or_default(),
            php: php.unwrap_or_default(),
            services,
        })
    }
}

#[test]
fn test_option_parse() {
    let mut args = vec![].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse::<_, &str>(&mut args).unwrap(),
        CloudOptions::default()
    );
    let mut args = vec!["mariadb"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            db: Database::MariaDB,
            ..Default::default()
        }
    );
    let mut args = vec!["rest"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            ..Default::default()
        }
    );
    let mut args = vec!["7"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            php: PhpVersion::Php74,
            ..Default::default()
        }
    );
    let mut args = vec!["7", "pgsql", "rest"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            php: PhpVersion::Php74,
            db: Database::Postgres,
            ..Default::default()
        }
    );
}

#[derive(Debug)]
pub struct Cloud {
    pub id: String,
    pub network: String,
    pub containers: Vec<String>,
    pub php: PhpVersion,
    pub db: Database,
    pub ip: Option<IpAddr>,
    pub workdir: Utf8PathBuf,
    pub services: Vec<Service>,
}

impl Cloud {
    pub async fn create(
        docker: &mut Docker,
        options: CloudOptions,
        config: &HazeConfig,
    ) -> Result<Self> {
        let id = format!("haze-{}", petname(2, "-"));

        let workdir = config.work_dir.join(&id);
        let mappings = default_mappings();
        for mapping in &mappings {
            mapping
                .create(&id, config)
                .await
                .wrap_err_with(|| format!("Failed to setup work directory {}", mapping.source))?;
        }

        let network = docker
            .create_network(CreateNetworkOptions {
                name: id.as_str(),
                ..Default::default()
            })
            .await?
            .id
            .ok_or(Report::msg("No network id in response"))
            .wrap_err("Failed to create network")?;

        let mut containers = Vec::new();

        let sources_meta = fs::metadata(&config.sources_root)?;
        let uid = sources_meta.uid();
        let gid = sources_meta.gid();

        let mut env = vec![
            "PHP_IDE_CONFIG=serverName=haze".to_string(),
            format!("UID={}", uid),
            format!("GID={}", gid),
            format!("SQL={}", options.db.name()),
        ];
        let volumes = mappings
            .into_iter()
            .filter_map(|mapping| mapping.get_volume_arg(&id, config))
            .collect();

        if let Some(db_name) = options
            .db
            .spawn(docker, &id, &network)
            .await
            .wrap_err("Failed to start database")?
        {
            containers.push(db_name);
            env.push(format!("SQL={}", options.db.name()));
        }

        let service_containers = try_join_all(
            options
                .services
                .iter()
                .map(|service| service.spawn(docker, &id, &network)),
        )
        .await?;
        containers.extend_from_slice(&service_containers);

        env.extend(
            options
                .services
                .iter()
                .flat_map(Service::env)
                .copied()
                .map(String::from),
        );

        let container = options
            .php
            .spawn(docker, &id, env, &options.db, &network, volumes)
            .await
            .wrap_err("Failed to start php container")?;

        let mut tries = 0;
        let ip = loop {
            let info = docker.inspect_container(&container, None).await?;
            if matches!(
                info.state,
                Some(ContainerState {
                    running: Some(true),
                    ..
                })
            ) {
                break info
                    .network_settings
                    .unwrap()
                    .networks
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()
                    .ip_address
                    .as_ref()
                    .unwrap()
                    .parse()
                    .unwrap();
            } else if tries > 100 {
                return Err(Report::msg("starting container timed out"));
            } else {
                tries += 1;
                sleep(Duration::from_millis(100)).await;
            }
        };

        containers.push(container);

        Ok(Cloud {
            id,
            network,
            containers,
            php: options.php,
            db: options.db,
            ip: Some(ip),
            workdir,
            services: options.services,
        })
    }

    pub async fn destroy(self, docker: &mut Docker) -> Result<()> {
        for container in self.containers {
            docker
                .remove_container(
                    container.trim_start_matches('/'),
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .wrap_err("Failed to remove container")?;
        }
        docker
            .remove_network(&self.network)
            .await
            .wrap_err("Failed to remove network")?;
        remove_dir_all(self.workdir)
            .await
            .wrap_err("Failed to remove work directory")?;

        Ok(())
    }

    pub async fn exec<S: Into<String>>(
        &self,
        docker: &mut Docker,
        cmd: Vec<S>,
        tty: bool,
    ) -> Result<i64> {
        if tty {
            exec_tty(docker, &self.id, "haze", cmd, vec![]).await
        } else {
            exec(docker, &self.id, "haze", cmd, vec![], Some(stdout())).await
        }
    }

    pub async fn list(
        docker: &mut Docker,
        filter: Option<String>,
        config: &HazeConfig,
    ) -> Result<Vec<Cloud>> {
        let containers = docker
            .list_containers::<String>(Some(ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await?;
        let mut containers_by_id: HashMap<String, (Option<_>, Vec<_>)> = HashMap::new();
        for container in containers {
            let labels = container.labels.clone().unwrap_or_default();
            if let Some(cloud_id) = labels.get("haze-cloud-id") {
                if match filter.as_ref() {
                    Some(filter) => cloud_id.contains(filter),
                    None => true,
                } {
                    let mut entry = containers_by_id.entry(cloud_id.to_string()).or_default();
                    if labels.get("haze-type").map(String::as_str) == Some("cloud") {
                        entry.0 = Some(container);
                    } else {
                        entry.1.push(container)
                    }
                }
            }
        }

        let mut sortable_containers: Vec<_> = containers_by_id
            .into_iter()
            .filter_map(|(id, (cloud, services))| {
                let cloud = cloud?;
                let network = id.clone();
                let networks = cloud.network_settings?.networks?;
                let network_info = networks.get(&network)?;
                let workdir = config.work_dir.join(&id);
                let labels = cloud.labels?;
                let db = labels.get("haze-db")?.parse().ok()?;
                let php = labels.get("haze-php")?.parse().ok()?;
                let found_services = services
                    .iter()
                    .flat_map(|container| &container.labels)
                    .flat_map(|labels| labels.get("haze-type"))
                    .map(String::as_str)
                    .flat_map(Service::from_type)
                    .collect();
                let mut service_ids: Vec<String> = services
                    .iter()
                    .filter_map(|service| service.names.as_ref()?.first().map(String::clone))
                    .collect();
                service_ids.push(id.clone());
                Some((
                    cloud.created.unwrap_or_default(),
                    Cloud {
                        id,
                        network,
                        db,
                        php,
                        containers: service_ids,
                        ip: network_info.ip_address.as_ref()?.parse().ok(),
                        workdir,
                        services: found_services,
                    },
                ))
            })
            .collect();

        sortable_containers.sort_by(|a, b| a.0.cmp(&b.0).reverse());

        Ok(sortable_containers
            .into_iter()
            .map(|(_created, cloud)| cloud)
            .collect())
    }

    pub async fn get_by_filter(
        docker: &mut Docker,
        filter: Option<String>,
        config: &HazeConfig,
    ) -> Result<Cloud> {
        Cloud::list(docker, filter, config)
            .await?
            .into_iter()
            .next()
            .ok_or(Report::msg("No clouds running matching filter"))
    }

    pub async fn wait_for_start(&self, docker: &mut Docker) -> Result<()> {
        self.php
            .wait_for_start(self.ip)
            .await
            .wrap_err("Failed to wait for php container")?;
        self.db
            .wait_for_start(docker, &self.id)
            .await
            .wrap_err("Failed to wait for database container")?;
        try_join_all(
            self.services
                .iter()
                .map(|service| service.wait_for_start(docker, &self.id)),
        )
        .await
        .wrap_err("Failed to wait for service containers")?;
        Ok(())
    }

    pub async fn enable_app<S: Into<String>>(&self, docker: &mut Docker, app: S) -> Result<()> {
        self.exec(
            docker,
            vec![
                "occ".to_string(),
                "app:enable".to_string(),
                app.into(),
                "--force".to_string(),
            ],
            false,
        )
        .await?;
        Ok(())
    }
}
