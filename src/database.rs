use crate::exec::{exec, exec_tty, ExitCode};
use crate::image::pull_image;
use bollard::container::{Config, CreateContainerOptions, NetworkingConfig};
use bollard::models::{EndpointSettings, HostConfig};
use bollard::Docker;
use maplit::hashmap;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use std::io::{stdout, Stdout};
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Eq, PartialEq)]
pub enum DatabaseFamily {
    Sqlite,
    Mysql,
    MariaDB,
    Postgres,
    Oracle,
}

impl DatabaseFamily {
    pub fn name(&self) -> &'static str {
        match self {
            DatabaseFamily::Sqlite => "sqlite",
            DatabaseFamily::Mysql => "mysql",
            DatabaseFamily::MariaDB => "mariadb",
            DatabaseFamily::Postgres => "pgsql",
            DatabaseFamily::Oracle => "oci",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
#[allow(dead_code)]
pub enum Database {
    #[default]
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
    Oracle,
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
            "pgsql:14" => Ok(Database::Postgres14),
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
            "oracle" => Ok(Database::Oracle),
            "oci" => Ok(Database::Oracle),
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
            Database::Oracle => "gvenzl/oracle-xe:21-faststart",
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
            Database::Oracle => DatabaseFamily::Oracle,
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
            DatabaseFamily::Oracle => vec!["ORACLE_PASSWORD=haze"],
        }
    }

    pub async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
    ) -> Result<Option<String>> {
        if matches!(self, Database::Sqlite) {
            return Ok(None);
        }
        if self.image().contains('/') {
            pull_image(docker, self.image())
                .await
                .wrap_err("Failed to pull database image")?;
        } else {
            pull_image(docker, &format!("library/{}", self.image()))
                .await
                .wrap_err("Failed to pull database image")?;
        }
        let options = Some(CreateContainerOptions {
            name: format!("{}-db", cloud_id),
            ..CreateContainerOptions::default()
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
        docker: &Docker,
        cloud_id: &str,
        cmd: Vec<S>,
        tty: bool,
    ) -> Result<ExitCode> {
        let container = match self.family() {
            DatabaseFamily::Sqlite => cloud_id.to_string(),
            _ => format!("{}-db", cloud_id),
        };
        if tty {
            exec_tty(docker, &container, "root", cmd, Vec::<String>::default()).await
        } else {
            exec(
                docker,
                &container,
                "root",
                cmd,
                Vec::<String>::default(),
                Some(stdout()),
            )
            .await
        }
    }

    pub async fn exec(&self, docker: &Docker, cloud_id: &str, root: bool) -> Result<ExitCode> {
        match self.family() {
            DatabaseFamily::Sqlite => {
                exec_tty(
                    docker,
                    cloud_id,
                    "haze",
                    vec!["sqlite3", "/var/www/html/data/haze.db"],
                    Vec::<String>::default(),
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
                    Vec::<String>::default(),
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
            DatabaseFamily::Oracle => {
                exec_tty(
                    docker,
                    format!("{}-db", cloud_id),
                    "root",
                    vec!["sqlplus", "system/haze"],
                    Vec::<String>::default(),
                )
                .await
            }
        }
    }

    pub async fn wait_for_start(&self, docker: &Docker, cloud_id: &str) -> Result<()> {
        let time = if self.family() == DatabaseFamily::Oracle {
            45
        } else {
            15
        };

        timeout(Duration::from_secs(time), async {
            while !self.is_healthy(docker, cloud_id).await? {
                sleep(Duration::from_millis(250)).await
            }
            Result::<(), Report>::Ok(())
        })
        .await
        .into_diagnostic()
        .wrap_err(format!("Timeout after {time} seconds"))?
    }

    pub async fn ip(&self, docker: &Docker, cloud_id: &str) -> Option<IpAddr> {
        match self.family() {
            DatabaseFamily::Sqlite => None,
            _ => docker
                .inspect_container(&format!("{}-db", cloud_id), None)
                .await
                .ok()?
                .network_settings?
                .networks?
                .values()
                .next()?
                .ip_address
                .clone()?
                .parse()
                .ok(),
        }
    }

    async fn is_healthy(&self, docker: &Docker, cloud_id: &str) -> Result<bool> {
        match self.family() {
            DatabaseFamily::Sqlite => Ok(true),
            DatabaseFamily::Mysql | DatabaseFamily::MariaDB => {
                let mut output = Vec::new();
                exec(
                    docker,
                    format!("{}-db", cloud_id),
                    "root",
                    vec!["mysql", "-u", "haze", "-phaze", "-e", "SELECT 1"],
                    Vec::<String>::default(),
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
                    Vec::<String>::default(),
                    Option::<Stdout>::None,
                )
                .await?;
                if is_ready_status == 0 {
                    let connect_status = exec(
                        docker,
                        format!("{}-db", cloud_id),
                        "root",
                        vec!["psql", "-U", "haze", "-qtA", "-c", ""],
                        Vec::<String>::default(),
                        Option::<Stdout>::None,
                    )
                    .await?;
                    Ok(connect_status == 0)
                } else {
                    Ok(false)
                }
            }
            DatabaseFamily::Oracle => {
                let mut output = Vec::new();
                exec(
                    docker,
                    format!("{}-db", cloud_id),
                    "root",
                    vec!["sh", "-c", r#"echo "show user" | sqlplus -S system/haze"#],
                    Vec::<String>::default(),
                    Some(&mut output),
                )
                .await?;
                let output = String::from_utf8(output).into_diagnostic()?;
                Ok(output.contains(r#"USER is "SYSTEM""#))
            }
        }
    }
}
