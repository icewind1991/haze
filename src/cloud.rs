use crate::config::HazeConfig;
use bollard::container::{Config, CreateContainerOptions, RemoveContainerOptions};
use bollard::models::{ContainerState, HostConfig};
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{eyre::WrapErr, Report, Result};
use maplit::hashmap;
use min_id::generate_id;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::fs::{create_dir_all, remove_dir_all, write};
use tokio::time::sleep;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Database {
    Sqlite,
    Mysql,
    Mysql80,
    Mysql57,
    Mysql56,
    MariaDB,
    MariaDB101,
    MariaDB102,
    MariaDB103,
    MariaDB104,
    MariaDB105,
    Postgres,
    Postgres9,
    Postgres10,
    Postgres11,
    Postgres12,
    Postgres13,
}

impl Default for Database {
    fn default() -> Self {
        Database::Sqlite
    }
}

impl FromStr for Database {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sqlite" => Ok(Database::Sqlite),
            "mysql" => Ok(Database::Mysql),
            "mariadb" => Ok(Database::MariaDB),
            "postgresql" => Ok(Database::Postgres),
            _ => Err(()),
        }
    }
}

impl Database {
    pub fn image(&self) -> &'static str {
        match self {
            Database::Sqlite => "",
            Database::Mysql => "mysql",
            Database::Mysql80 => "mysql:8",
            Database::Mysql57 => "mysql:5.7",
            Database::Mysql56 => "mysql:5.6",
            Database::MariaDB => "mariadb",
            Database::MariaDB101 => "mariadb:10.1",
            Database::MariaDB102 => "mariadb:10.2",
            Database::MariaDB103 => "mariadb:10.3",
            Database::MariaDB104 => "mariadb:10.4",
            Database::MariaDB105 => "mariadb:10.5",
            Database::Postgres => "postgresql",
            Database::Postgres9 => "postgresql:9",
            Database::Postgres10 => "postgresql:10",
            Database::Postgres11 => "postgresql:11",
            Database::Postgres12 => "postgresql:12",
            Database::Postgres13 => "postgresql:13",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Database::Sqlite => "sqlite",
            Database::Mysql
            | Database::Mysql80
            | Database::Mysql57
            | Database::Mysql56
            | Database::MariaDB
            | Database::MariaDB101
            | Database::MariaDB102
            | Database::MariaDB103
            | Database::MariaDB104
            | Database::MariaDB105 => "mysql",
            Database::Postgres
            | Database::Postgres9
            | Database::Postgres10
            | Database::Postgres11
            | Database::Postgres12
            | Database::Postgres13 => "pgsql",
        }
    }

    pub fn env(&self) -> Vec<&'static str> {
        match self {
            Database::Sqlite => Vec::new(),
            Database::Mysql
            | Database::Mysql80
            | Database::Mysql57
            | Database::Mysql56
            | Database::MariaDB
            | Database::MariaDB101
            | Database::MariaDB102
            | Database::MariaDB103
            | Database::MariaDB104
            | Database::MariaDB105 => vec![
                "MYSQL_ROOT_PASSWORD=haze",
                "MYSQL_PASSWORD=haze",
                "MYSQL_USER=haze",
                "MYSQL_DATABASE=haze",
            ],
            Database::Postgres
            | Database::Postgres9
            | Database::Postgres10
            | Database::Postgres11
            | Database::Postgres12
            | Database::Postgres13 => vec![
                "POSTGRES_PASSWORD=haze",
                "POSTGRES_USER=haze",
                "POSTGRES_DATABASE=haze",
            ],
        }
    }

    pub async fn spawn(
        &self,
        docker: &mut Docker,
        cloud_id: &str,
        network: &str,
    ) -> Result<Option<String>> {
        if matches!(self, Database::Sqlite) {
            return Ok(None);
        }
        let options = Some(CreateContainerOptions {
            name: format!("{}-db", cloud_id),
        });
        let config = Config {
            image: Some(self.image()),
            env: Some(self.env()),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                ..Default::default()
            }),
            labels: Some(hashmap! {
                "haze-type" => "db",
                "haze-cloud-id" => cloud_id
            }),
            ..Default::default()
        };
        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;
        Ok(Some(id))
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum PhpVersion {
    Latest,
    // Php80,
    Php74,
    // Php73,
}

impl PhpVersion {
    fn image(&self) -> &'static str {
        // for now only 7.4
        match self {
            PhpVersion::Latest => "icewind1991/nextcloud-dev:7",
            PhpVersion::Php74 => "icewind1991/nextcloud-dev:7",
        }
    }

    async fn spawn(
        &self,
        docker: &mut Docker,
        id: &str,
        env: Vec<String>,
        db: &Database,
        network: &str,
        links: Vec<String>,
        volumes: Vec<String>,
    ) -> Result<String> {
        let options = Some(CreateContainerOptions {
            name: id.to_string(),
        });
        let config = Config {
            image: Some(self.image().to_string()),
            env: Some(env),
            host_config: Some(HostConfig {
                network_mode: Some(network.to_string()),
                links: Some(links),
                binds: Some(volumes),
                ..Default::default()
            }),
            labels: Some(hashmap! {
                "haze-type".to_string() => "cloud".to_string(),
                "haze-db".to_string() => db.name().to_string(),
                "haze-cloud-id".to_string() => id.to_string(),
            }),
            ..Default::default()
        };

        let id = docker.create_container(options, config).await?.id;
        docker.start_container::<String>(&id, None).await?;
        Ok(id)
    }
}

impl Default for PhpVersion {
    fn default() -> Self {
        PhpVersion::Latest
    }
}

#[derive(Default, Debug)]
pub struct CloudOptions {
    db: Database,
    php: PhpVersion,
}

#[derive(Debug)]
pub struct Cloud {
    pub id: String,
    network: String,
    containers: Vec<String>,
    db: Database,
    pub ip: IpAddr,
    workdir: Utf8PathBuf,
}

impl Cloud {
    pub async fn create(
        docker: &mut Docker,
        options: CloudOptions,
        config: &HazeConfig,
    ) -> Result<Self> {
        let id = format!("haze-{}", generate_id());

        let workdir = setup_workdir(&config.work_dir, &id)
            .await
            .wrap_err("Failed to setup work directories")?;

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
        let mut links = Vec::new();
        let mut env = vec!["PHP_IDE_CONFIG=serverName=haze".to_string()];
        let volumes = vec![
            format!("{}:/var/www/html", config.sources_root),
            format!("{}/{}/data:/var/www/html/data", config.work_dir, id),
            format!("{}/{}/config:/var/www/html/config", config.work_dir, id),
            format!(
                "{}/{}/data-autotest:/var/www/html/data-autotest",
                config.work_dir, id
            ),
            format!(
                "{}/{}/skeleton:/var/www/html/core/skeleton",
                config.work_dir, id
            ),
            format!(
                "{}/{}/skeleton/welcome.txt:/var/www/html/core/skeleton/welcome.txt:ro",
                config.sources_root, id
            ),
            format!(
                "{}/{}/integration/vendor:/var/www/html/build/integration/vendor",
                config.work_dir, id
            ),
            format!(
                "{}/{}/integration/work:/var/www/html/build/integration/work",
                config.work_dir, id
            ),
            format!(
                "{}/{}/integration/output:/var/www/html/build/integration/output",
                config.work_dir, id
            ),
            format!(
                "{}/{}/integration/composer.lock:/var/www/html/build/integration/composer.lock",
                config.work_dir, id
            ),
            format!(
                "{}/composer/cache:/var/www/.composer/cache",
                config.work_dir
            ),
            format!(
                "{}/phpunit-cache:/var/www/html/tests/.phpunit.results.cache",
                config.work_dir
            ),
        ];

        if let Some(db_name) = options
            .db
            .spawn(docker, &id, &network)
            .await
            .wrap_err("Failed to start database")?
        {
            containers.push(db_name);
            links.push(format!("{}-db:{}", id, options.db.name()));
            env.push(format!("SQL={}", options.db.name()));
        }

        let container = options
            .php
            .spawn(docker, &id, env, &options.db, &network, links, volumes)
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
            db: options.db,
            ip,
            workdir,
        })
    }

    #[allow(dead_code)]
    pub async fn destroy(self, docker: &mut Docker) -> Result<()> {
        for container in self.containers {
            docker
                .remove_container(
                    &container,
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
}

async fn setup_workdir(base: &Utf8Path, id: &str) -> Result<Utf8PathBuf> {
    let workdir = base.join(id);
    create_dir_all(workdir.join("data")).await?;
    create_dir_all(workdir.join("config")).await?;
    create_dir_all(workdir.join("data-autotest")).await?;
    create_dir_all(workdir.join("skeleton")).await?;
    create_dir_all(workdir.join("integration/output")).await?;
    create_dir_all(workdir.join("integration/work")).await?;
    create_dir_all(workdir.join("integration/vendor")).await?;

    write(workdir.join("integration/composer.lock"), "").await?;
    write(workdir.join("config/CAN_INSTALL"), "").await?;
    write(workdir.join("phpunit-cache"), "").await?;

    create_dir_all(base.join("composer/cache")).await?;

    Ok(workdir)
}

pub async fn parse(docker: &mut Docker, config: &HazeConfig) -> Result<Vec<Cloud>> {
    let containers = docker.list_containers::<String>(None).await?;
    let mut containers_by_id: HashMap<String, (Option<_>, Vec<_>)> = HashMap::new();
    for container in containers {
        let labels = container.labels.clone().unwrap_or_default();
        if let Some(cloud_id) = labels.get("haze-cloud-id") {
            let mut entry = containers_by_id.entry(cloud_id.to_string()).or_default();
            if labels.get("haze-type").map(String::as_str) == Some("cloud") {
                entry.0 = Some(container);
            } else {
                entry.1.push(container)
            }
        }
    }

    Ok(containers_by_id
        .into_iter()
        .filter_map(|(id, (cloud, services))| {
            let cloud = cloud?;
            let network = id.clone();
            let networks = cloud.network_settings?.networks?;
            let network_info = networks.get(&network)?;
            let workdir = config.work_dir.join(&id);
            let db = cloud.labels?.get("haze-db")?.parse().ok()?;
            let mut service_ids: Vec<String> = services
                .iter()
                .filter_map(|service| service.names.as_ref()?.first().map(String::clone))
                .collect();
            service_ids.push(id.clone());
            Some(Cloud {
                id,
                network,
                db,
                containers: service_ids,
                ip: network_info.ip_address.as_ref()?.parse().ok()?,
                workdir,
            })
        })
        .collect())
}
