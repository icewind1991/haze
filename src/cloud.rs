use crate::config::{HazeConfig, HazeVolumeConfig};
use crate::database::Database;
use crate::exec::{exec, exec_tty, ExitCode};
use crate::mapping::{default_mappings, Mapping};
use crate::php::PhpVersion;
use crate::service::Service;
use crate::service::ServiceTrait;
use bollard::container::{ListContainersOptions, RemoveContainerOptions};
use bollard::models::ContainerState;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::Utf8PathBuf;
use color_eyre::{eyre::WrapErr, Report, Result};
use flate2::read::GzDecoder;
use futures_util::future::try_join_all;
use petname::petname;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::io::{stdout, Write};
use std::iter::Peekable;
use std::net::IpAddr;
use std::os::unix::fs::MetadataExt;
use std::str::FromStr;
use std::time::Duration;
use tokio::fs::create_dir_all;
use tokio::fs::remove_dir_all;
use tokio::task::spawn;
use tokio::time::sleep;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct CloudOptions {
    db: Database,
    php: PhpVersion,
    services: Vec<Service>,
    app_packages: Vec<Utf8PathBuf>,
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
        let mut app_package = Vec::new();

        while let Some(option) = args.peek() {
            if let Ok(db_option) = Database::from_str(option.as_ref()) {
                db = Some(db_option);
                let _ = args.next();
            } else if let Ok(php_option) = PhpVersion::from_str(option.as_ref()) {
                php = Some(php_option);
                let _ = args.next();
            } else if let Some(service) = Service::from_type(option.as_ref()) {
                services.extend_from_slice(service);
                let _ = args.next();
            } else if option.as_ref().ends_with(".tar.gz") {
                app_package.push(option.to_string().into());
                let _ = args.next();
            } else {
                break;
            }
        }

        Ok(CloudOptions {
            db: db.unwrap_or_default(),
            php: php.unwrap_or_default(),
            services,
            app_packages: app_package,
        })
    }
}

#[test]
fn test_option_parse() {
    use crate::service::{LDAPAdmin, LDAP};

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
    let mut args = vec!["7", "ldap", "pgsql"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            php: PhpVersion::Php74,
            db: Database::Postgres,
            services: vec![Service::LDAP(LDAP), Service::LDAPAdmin(LDAPAdmin)],
            ..Default::default()
        }
    );
    let mut args = vec!["7", "pgsql", "ldap"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            php: PhpVersion::Php74,
            db: Database::Postgres,
            services: vec![Service::LDAP(LDAP), Service::LDAPAdmin(LDAPAdmin)],
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
        let app_package_dir = workdir.join("app_package");

        if !options.app_packages.is_empty() {
            create_dir_all(&app_package_dir)
                .await
                .wrap_err("Failed to create directory for app packages")?;
        }

        let app_volumes = options
            .app_packages
            .into_iter()
            .map(|app_package| {
                let app_name = app_package.file_name().unwrap().trim_end_matches(".tar.gz");
                let app_dir = app_package_dir.join(app_name);

                let app_package_file = fs::File::open(&app_package)
                    .wrap_err_with(|| format!("Failed to open app bundle {}", app_package))?;
                if app_package.metadata()?.len() > 1024 * 1024 {
                    println!("Extracting app archive for {}...", app_name);
                }
                let gz = GzDecoder::new(app_package_file);
                tar::Archive::new(gz)
                    .unpack(&app_package_dir)
                    .wrap_err_with(|| format!("Failed to extract app bundle {}", app_package))?;

                Ok(HazeVolumeConfig {
                    create: false,
                    source: app_dir,
                    read_only: true,
                    target: format!("/var/www/html/apps/{}", app_name).into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mappings = config
            .volume
            .iter()
            .map(Mapping::from)
            .chain(default_mappings())
            .chain(app_volumes.iter().map(Mapping::from))
            .collect::<Vec<_>>();
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

        let network_info = docker.inspect_network::<String>(&network, None).await?;
        let gateway = network_info
            .ipam
            .as_ref()
            .ok_or(Report::msg("Network has no ip info"))?
            .config
            .as_deref()
            .ok_or(Report::msg("Network has no ip info"))?
            .first()
            .ok_or(Report::msg("Network has no ip info"))?
            .get("Gateway")
            .ok_or(Report::msg("Network has no ip info"))?;

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
                .map(|service| service.spawn(docker, &id, &network, config)),
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
            .spawn(
                docker,
                &id,
                env,
                &options.db,
                &network,
                volumes,
                gateway.as_str(),
            )
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
                    .iter()
                    .filter_map(|(name, network)| name.eq("haze").then(|| network))
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

        let services_clone = options.services.clone();
        let cloud_id = id.clone();
        let docker_clone = docker.clone();
        spawn(async move {
            if let Err(e) = try_join_all(
                services_clone
                    .iter()
                    .map(|service| service.wait_for_start(&docker_clone, &cloud_id)),
            )
            .await
            {
                println!("{:#}", e);
                return;
            }
            for service in services_clone {
                match service.start_message(&docker_clone, &cloud_id).await {
                    Ok(Some(msg)) => {
                        println!("{}", msg);
                    }
                    Err(e) => {
                        println!("{:#}", e);
                    }
                    _ => {}
                }
            }
        });

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
        if self.workdir.exists() {
            if let Err(e) = remove_dir_all(self.workdir)
                .await
                .wrap_err("Failed to remove work directory")
            {
                eprintln!("{}", e);
            }
        }

        Ok(())
    }

    pub async fn exec<S: Into<String>>(
        &self,
        docker: &mut Docker,
        cmd: Vec<S>,
        tty: bool,
    ) -> Result<ExitCode> {
        if tty {
            exec_tty(docker, &self.id, "haze", cmd, vec![]).await
        } else {
            exec(docker, &self.id, "haze", cmd, vec![], Some(stdout())).await
        }
    }

    pub async fn occ<'a, S: Into<String> + From<&'a str>>(
        &self,
        docker: &Docker,
        mut cmd: Vec<S>,
        output: Option<&mut Vec<u8>>,
    ) -> Result<ExitCode> {
        cmd.insert(0, "occ".into());
        self.exec_with_output(docker, cmd, output).await
    }

    pub async fn exec_with_output<S: Into<String>>(
        &self,
        docker: &Docker,
        cmd: Vec<S>,
        output: Option<impl Write>,
    ) -> Result<ExitCode> {
        exec(docker, &self.id, "haze", cmd, vec![], output).await
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
                let network_info = networks.get("haze")?;
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
                    .flatten()
                    .cloned()
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
