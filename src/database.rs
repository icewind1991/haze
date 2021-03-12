use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use color_eyre::{Report, Result};
use maplit::hashmap;
use std::str::FromStr;

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
