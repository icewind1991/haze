use crate::config::HazeConfig;
use bollard::container::{Config, CreateContainerOptions};
use bollard::models::HostConfig;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::Utf8Path;
use color_eyre::{eyre::WrapErr, Report, Result};
use min_id::generate_id;
use tokio::fs::{create_dir_all, write};

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
            ..Default::default()
        };
        Ok(Some(docker.create_container(options, config).await?.id))
    }
}

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
            ..Default::default()
        };
        Ok(docker.create_container(options, config).await?.id)
    }
}

impl Default for PhpVersion {
    fn default() -> Self {
        PhpVersion::Latest
    }
}

#[derive(Default)]
pub struct CloudOptions {
    db: Database,
    php: PhpVersion,
}

pub struct Cloud {
    id: String,
    network: String,
    containers: Vec<String>,
}

impl Cloud {
    pub async fn create(
        docker: &mut Docker,
        options: CloudOptions,
        config: HazeConfig,
    ) -> Result<Self> {
        let id = generate_id();

        setup_workdir(&config.work_dir, &id)
            .await
            .wrap_err("Failed to setup work directories")?;

        let network = docker
            .create_network(CreateNetworkOptions {
                name: format!("cloud-{}", id),
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
                config.work_dir, id
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

        if let Some(db) = options
            .db
            .spawn(docker, &id, &network)
            .await
            .wrap_err("Failed to start database")?
        {
            containers.push(db);
            links.push(format!("{}-db:{}", id, options.db.name()));
            env.push(format!("SQL={}", options.db.name()));
        }

        let container = options
            .php
            .spawn(docker, &id, env, &network, links, volumes)
            .await
            .wrap_err("Failed to start php container")?;
        containers.push(container);

        Ok(Cloud {
            id,
            network,
            containers,
        })
    }
}

async fn setup_workdir(base: &Utf8Path, id: &str) -> Result<()> {
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

    Ok(())
}
