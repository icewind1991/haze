use crate::config::HazeConfig;
use crate::database::Database;
use crate::php::PhpVersion;
use bollard::container::{ListContainersOptions, LogsOptions, RemoveContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::ContainerState;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{eyre::WrapErr, Report, Result};
use futures_util::stream::StreamExt;
use min_id::generate_id;
use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::os::unix::fs::MetadataExt;
use std::str::FromStr;
use std::time::Duration;
use tokio::fs::{create_dir_all, remove_dir_all, write};
use tokio::time::sleep;

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
                "{}/skeleton/welcome.txt:/var/www/html/core/skeleton/welcome.txt:ro",
                config.sources_root
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

    pub async fn exec(&self, docker: &mut Docker, cmd: Vec<String>) -> Result<()> {
        let config = CreateExecOptions {
            cmd: Some(cmd),
            user: Some("haze".to_string()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        let message = docker.create_exec(&self.id, config).await?;
        let mut exec = docker.start_exec(&message.id, None);
        while let Some(res) = exec.next().await {
            match res? {
                StartExecResults::Attached { log } => {
                    print!("{}", log);
                }
                _ => {}
            }
        }
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
