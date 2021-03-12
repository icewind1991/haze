use crate::config::HazeConfig;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, LogsOptions, NetworkingConfig,
    RemoveContainerOptions,
};
use bollard::models::{ContainerState, EndpointSettings, HostConfig};
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{eyre::WrapErr, Report, Result};
use futures_util::stream::StreamExt;
use maplit::hashmap;
use min_id::generate_id;
use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::os::unix::fs::MetadataExt;
use std::str::FromStr;
use std::time::Duration;
use tokio::fs::{create_dir_all, remove_dir_all, write};
use tokio::time::sleep;

#[derive(Debug, Eq, PartialEq)]
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
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sqlite" => Ok(Database::Sqlite),
            "mysql" => Ok(Database::Mysql),
            "mysql:8" => Ok(Database::Mysql80),
            "mysql:5" => Ok(Database::Mysql57),
            "mysql:5.7" => Ok(Database::Mysql57),
            "mysql:5.6" => Ok(Database::Mysql56),
            "mariadb" => Ok(Database::MariaDB),
            "mariadb:10.1" => Ok(Database::MariaDB101),
            "mariadb:10.2" => Ok(Database::MariaDB102),
            "mariadb:10.3" => Ok(Database::MariaDB103),
            "mariadb:10.4" => Ok(Database::MariaDB104),
            "mariadb:10.5" => Ok(Database::MariaDB105),
            "mariadb:10" => Ok(Database::MariaDB105),
            "pgsql" => Ok(Database::Postgres),
            "pgsql:9" => Ok(Database::Postgres9),
            "pgsql:10" => Ok(Database::Postgres10),
            "pgsql:11" => Ok(Database::Postgres11),
            "pgsql:12" => Ok(Database::Postgres12),
            "pgsql:13" => Ok(Database::Postgres13),
            _ => Err(Report::msg("Unknown db type")),
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
            Database::Postgres => "postgres",
            Database::Postgres9 => "postgres:9",
            Database::Postgres10 => "postgres:10",
            Database::Postgres11 => "postgres:11",
            Database::Postgres12 => "postgres:12",
            Database::Postgres13 => "postgres:13",
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
        Ok(Some(id))
    }
}

#[derive(Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum PhpVersion {
    Latest,
    // Php80,
    Php74,
    // Php73,
}

impl FromStr for PhpVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "7" => Ok(PhpVersion::Php74),
            "7.4" => Ok(PhpVersion::Php74),
            _ => Err(()),
        }
    }
}

impl PhpVersion {
    fn image(&self) -> &'static str {
        // for now only 7.4
        match self {
            PhpVersion::Latest => "icewind1991/haze:7.4",
            PhpVersion::Php74 => "icewind1991/haze:7.4",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PhpVersion::Latest => "7.4",
            PhpVersion::Php74 => "7.4",
        }
    }

    async fn spawn(
        &self,
        docker: &mut Docker,
        id: &str,
        env: Vec<String>,
        db: &Database,
        network: &str,
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
                // links: Some(links),
                binds: Some(volumes),
                ..Default::default()
            }),
            networking_config: Some(NetworkingConfig {
                endpoints_config: hashmap! {
                    network.to_string() => EndpointSettings {
                        aliases: Some(vec!["cloud".to_string()]),
                        ..Default::default()
                    }
                },
            }),
            labels: Some(hashmap! {
                "haze-type".to_string() => "cloud".to_string(),
                "haze-db".to_string() => db.name().to_string(),
                "haze-php".to_string() => self.name().to_string(),
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

#[derive(Default, Debug, Eq, PartialEq)]
pub struct CloudOptions {
    db: Database,
    php: PhpVersion,
}

impl CloudOptions {
    pub fn parse<S>(options: Vec<S>) -> Result<(CloudOptions, Vec<S>)>
    where
        S: AsRef<str> + Clone,
    {
        let mut db = Database::default();
        let mut php = PhpVersion::default();
        let mut used = 0;

        for option in options.iter() {
            if let Ok(db_option) = Database::from_str(option.as_ref()) {
                db = db_option;
                used += 1;
                continue;
            }
            if let Ok(php_option) = PhpVersion::from_str(option.as_ref()) {
                php = php_option;
                used += 1;
                continue;
            }
        }

        let rest = options[used..].to_vec();

        Ok((CloudOptions { db, php }, rest))
    }
}

#[test]
fn test_option_parse() {
    assert_eq!(
        CloudOptions::parse::<&str>(vec![]).unwrap(),
        (CloudOptions::default(), vec![])
    );
    assert_eq!(
        CloudOptions::parse(vec!["mariadb"]).unwrap(),
        (
            CloudOptions {
                db: Database::MariaDB,
                ..Default::default()
            },
            vec![]
        )
    );
    assert_eq!(
        CloudOptions::parse(vec!["rest"]).unwrap(),
        (
            CloudOptions {
                ..Default::default()
            },
            vec!["rest"]
        )
    );
    assert_eq!(
        CloudOptions::parse(vec!["7"]).unwrap(),
        (
            CloudOptions {
                php: PhpVersion::Php74,
                ..Default::default()
            },
            vec![]
        )
    );
    assert_eq!(
        CloudOptions::parse(vec!["7", "pgsql", "rest"]).unwrap(),
        (
            CloudOptions {
                php: PhpVersion::Php74,
                db: Database::Postgres,
                ..Default::default()
            },
            vec!["rest"]
        )
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

        let sources_meta = fs::metadata(&config.sources_root)?;
        let uid = sources_meta.uid();
        let gid = sources_meta.gid();

        let mut env = vec![
            "PHP_IDE_CONFIG=serverName=haze".to_string(),
            format!("UID={}", uid),
            format!("GID={}", gid),
        ];
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
            env.push(format!("SQL={}", options.db.name()));
        }

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

    pub async fn logs(&self, docker: &mut Docker) -> Result<Vec<String>> {
        let mut logs = Vec::new();
        let mut stream = docker.logs::<String>(
            &self.id,
            Some(LogsOptions {
                stdout: true,
                stderr: true,
                ..Default::default()
            }),
        );
        while let Some(line) = stream.next().await {
            logs.push(line?.to_string());
        }
        Ok(logs)
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
    list(docker, filter, config)
        .await?
        .into_iter()
        .next()
        .ok_or(Report::msg("No clouds running matching filter"))
}
