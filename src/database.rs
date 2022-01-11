use crate::exec::{exec, exec_tty, ExitCode};
use crate::image::pull_image;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use std::io::{stdout, Stdout};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{sleep, timeout};

pub enum DatabaseFamily {
    Sqlite,
    Mysql,
    MariaDB,
    Postgres,
}

impl DatabaseFamily {
    pub fn name(&self) -> &'static str {
        match self {
            DatabaseFamily::Sqlite => "sqlite",
            DatabaseFamily::Mysql => "mysql",
            DatabaseFamily::MariaDB => "mariadb",
            DatabaseFamily::Postgres => "pgsql",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    Postgres14,
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
            "postgres" => Ok(Database::Postgres),
            "postgres:9" => Ok(Database::Postgres9),
            "postgres:10" => Ok(Database::Postgres10),
            "postgres:11" => Ok(Database::Postgres11),
            "postgres:12" => Ok(Database::Postgres12),
            "postgres:13" => Ok(Database::Postgres13),
            "postgresql" => Ok(Database::Postgres),
            "postgresql:9" => Ok(Database::Postgres9),
            "postgresql:10" => Ok(Database::Postgres10),
            "postgresql:11" => Ok(Database::Postgres11),
            "postgresql:12" => Ok(Database::Postgres12),
            "postgresql:13" => Ok(Database::Postgres13),
            "postgresql:14" => Ok(Database::Postgres14),
            _ => Err(Report::msg("Unknown db type")),
        }
    }
}

impl Database {
    pub fn image(&self) -> &'static str {
        match self {
            Database::Sqlite => "",
            Database::Mysql => "mysql:8",
            Database::Mysql80 => "mysql:8",
            Database::Mysql57 => "mysql:5.7",
            Database::Mysql56 => "mysql:5.6",
            Database::MariaDB => "mariadb:10",
            Database::MariaDB101 => "mariadb:10.1",
            Database::MariaDB102 => "mariadb:10.2",
            Database::MariaDB103 => "mariadb:10.3",
            Database::MariaDB104 => "mariadb:10.4",
            Database::MariaDB105 => "mariadb:10.5",
            Database::Postgres => "postgres:14",
            Database::Postgres9 => "postgres:9",
            Database::Postgres10 => "postgres:10",
            Database::Postgres11 => "postgres:11",
            Database::Postgres12 => "postgres:12",
            Database::Postgres13 => "postgres:13",
            Database::Postgres14 => "postgres:14",
        }
    }

    pub fn name(&self) -> &str {
        self.family().name()
    }

    pub fn family(&self) -> DatabaseFamily {
        match self {
            Database::Sqlite => DatabaseFamily::Sqlite,
            Database::Mysql | Database::Mysql80 | Database::Mysql57 | Database::Mysql56 => {
                DatabaseFamily::Mysql
            }
            Database::MariaDB
            | Database::MariaDB101
            | Database::MariaDB102
            | Database::MariaDB103
            | Database::MariaDB104
            | Database::MariaDB105 => DatabaseFamily::MariaDB,
            Database::Postgres
            | Database::Postgres9
            | Database::Postgres10
            | Database::Postgres11
            | Database::Postgres12
            | Database::Postgres13
            | Database::Postgres14 => DatabaseFamily::Postgres,
        }
    }

    pub fn env(&self) -> Vec<&'static str> {
        match self.family() {
            DatabaseFamily::Sqlite => Vec::new(),
            DatabaseFamily::Mysql | DatabaseFamily::MariaDB => vec![
                "MYSQL_ROOT_PASSWORD=haze",
                "MYSQL_PASSWORD=haze",
                "MYSQL_USER=haze",
                "MYSQL_DATABASE=haze",
            ],
            DatabaseFamily::Postgres => vec![
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
        pull_image(docker, self.image()).await?;
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
            cmd: if self.image() == "mysql:8" {
                Some(vec![
                    "--default-authentication-plugin",
                    "mysql_native_password",
                ])
            } else {
                None
            },
            ..Default::default()
        };
        let id = docker
            .create_container(options, config)
            .await
            .into_diagnostic()?
            .id;
        docker
            .start_container::<String>(&id, None)
            .await
            .into_diagnostic()?;
        Ok(Some(id))
    }

    pub async fn exec_sh<S: Into<String>>(
        &self,
        docker: &mut Docker,
        cloud_id: &str,
        cmd: Vec<S>,
        tty: bool,
    ) -> Result<ExitCode> {
        let container = match self.family() {
            DatabaseFamily::Sqlite => cloud_id.to_string(),
            _ => format!("{}-db", cloud_id.to_string()),
        };
        if tty {
            exec_tty(docker, &container, "root", cmd, vec![]).await
        } else {
            exec(docker, &container, "root", cmd, vec![], Some(stdout())).await
        }
    }

    pub async fn exec(&self, docker: &mut Docker, cloud_id: &str, root: bool) -> Result<ExitCode> {
        match self.family() {
            DatabaseFamily::Sqlite => {
                exec_tty(
                    docker,
                    cloud_id,
                    "haze",
                    vec!["sqlite3", "/var/www/html/data/haze.db"],
                    vec![],
                )
                .await
            }
            DatabaseFamily::MariaDB | DatabaseFamily::Mysql => {
                exec_tty(
                    docker,
                    format!("{}-db", cloud_id),
                    "mysql",
                    vec![
                        "mysql",
                        "-u",
                        if root { "root" } else { "haze" },
                        "-phaze",
                        "haze",
                    ],
                    vec![],
                )
                .await
            }
            DatabaseFamily::Postgres => {
                exec_tty(
                    docker,
                    format!("{}-db", cloud_id),
                    "root",
                    vec!["psql", "haze", "haze"],
                    vec!["PGPASSWORD=haze"],
                )
                .await
            }
        }
    }

    pub async fn wait_for_start(&self, docker: &mut Docker, cloud_id: &str) -> Result<()> {
        timeout(Duration::from_secs(15), async {
            while !self.is_healthy(docker, cloud_id).await? {
                sleep(Duration::from_millis(100)).await
            }
            Ok(())
        })
        .await
        .into_diagnostic()
        .wrap_err("Timeout after 15 seconds")?
    }

    async fn is_healthy(&self, docker: &mut Docker, cloud_id: &str) -> Result<bool> {
        match self.family() {
            DatabaseFamily::Sqlite => Ok(true),
            DatabaseFamily::Mysql | DatabaseFamily::MariaDB => {
                let mut output = Vec::new();
                exec(
                    docker,
                    format!("{}-db", cloud_id),
                    "root",
                    vec!["mysql", "-u", "haze", "-phaze", "-e", "SELECT 1"],
                    vec![],
                    Some(&mut output),
                )
                .await?;
                let output = String::from_utf8(output).into_diagnostic()?;
                Ok(!output.contains("ERROR"))
            }
            DatabaseFamily::Postgres => {
                let is_ready_status = exec(
                    docker,
                    format!("{}-db", cloud_id),
                    "root",
                    vec!["pg_isready", "-U", "haze", "-q"],
                    vec![],
                    Option::<Stdout>::None,
                )
                .await?;
                if is_ready_status == 0 {
                    let connect_status = exec(
                        docker,
                        format!("{}-db", cloud_id),
                        "root",
                        vec!["psql", "-U", "haze", "-qtA", "-c", ""],
                        vec![],
                        Option::<Stdout>::None,
                    )
                    .await?;
                    Ok(connect_status == 0)
                } else {
                    Ok(false)
                }
            }
        }
    }
}
