use crate::config::{HazeConfig, HazeVolumeConfig, Preset};
use crate::database::Database;
use crate::exec::{exec, exec_tty, ExitCode};
use crate::mapping::{default_mappings, Mapping};
use crate::php::{PhpVersion, PHP_MEMORY_LIMIT};
use crate::service::Service;
use crate::service::ServiceTrait;
use bollard::container::{ListContainersOptions, RemoveContainerOptions, UpdateContainerOptions};
use bollard::models::ContainerState;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use camino::Utf8PathBuf;
use flate2::read::GzDecoder;
use futures_util::future::try_join_all;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
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
    name: Option<String>,
    db: Database,
    php: PhpVersion,
    services: Vec<Service>,
    app_packages: Vec<Utf8PathBuf>,
}

impl CloudOptions {
    pub fn parse<I, S>(presets: &[Preset], args: &mut Peekable<I>) -> Result<CloudOptions>
    where
        S: AsRef<str> + Into<String> + Display,
        I: Iterator<Item = S>,
    {
        let mut db = None;
        let mut php = None;
        let mut name = None;
        let mut services = Vec::new();
        let mut app_package = Vec::new();

        while let Some(option) = args.peek() {
            if let Ok(db_option) = Database::from_str(option.as_ref()) {
                db = Some(db_option);
                let _ = args.next();
            } else if let Ok(php_option) = PhpVersion::from_str(option.as_ref()) {
                php = Some(php_option);
                let _ = args.next();
            } else if let Some(service) = Service::from_type(presets, option.as_ref()) {
                services.extend_from_slice(&service);
                let _ = args.next();
            } else if option.as_ref().ends_with(".tar.gz") {
                app_package.push(option.to_string().into());
                let _ = args.next();
            } else if option.as_ref() == "--name" {
                let _ = args.next();
                name = args.next().map(|s| s.into());
            } else {
                break;
            }
        }

        Ok(CloudOptions {
            name,
            db: db.unwrap_or_default(),
            php: php.unwrap_or_default(),
            services,
            app_packages: app_package,
        })
    }
}

#[test]
fn test_option_parse() {
    use crate::service::{Ldap, LdapAdmin};

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
            services: vec![Service::Ldap(Ldap), Service::LdapAdmin(LdapAdmin)],
            ..Default::default()
        }
    );
    let mut args = vec!["7", "pgsql", "ldap"].into_iter().peekable();
    assert_eq!(
        CloudOptions::parse(&mut args).unwrap(),
        CloudOptions {
            php: PhpVersion::Php74,
            db: Database::Postgres,
            services: vec![Service::Ldap(Ldap), Service::LdapAdmin(LdapAdmin)],
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
    pub pinned: bool,
    pub address: String,
}

impl Cloud {
    pub async fn create(
        docker: &Docker,
        options: CloudOptions,
        config: &HazeConfig,
    ) -> Result<Self> {
        let id = options
            .name
            .map(|name| format!("haze-{}", name))
            .unwrap_or_else(|| format!("haze-{}", petname(2, "-")));

        let workdir = config.work_dir.join(&id);
        let app_package_dir = workdir.join("app_package");

        if !options.app_packages.is_empty() {
            create_dir_all(&app_package_dir)
                .await
                .into_diagnostic()
                .wrap_err("Failed to create directory for app packages")?;
        }

        let app_volumes = options
            .app_packages
            .into_iter()
            .map(|app_package| {
                let app_name = app_package.file_name().unwrap().trim_end_matches(".tar.gz");
                let app_dir = app_package_dir.join(app_name);

                let app_package_file = fs::File::open(&app_package)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("Failed to open app bundle {}", app_package))?;
                if app_package.metadata().into_diagnostic()?.len() > 1024 * 1024 {
                    println!("Extracting app archive for {}...", app_name);
                }
                let gz = GzDecoder::new(app_package_file);
                tar::Archive::new(gz)
                    .unpack(&app_package_dir)
                    .into_diagnostic()
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
            .await
            .into_diagnostic()?
            .id
            .ok_or_else(|| Report::msg("No network id in response"))
            .wrap_err("Failed to create network")?;

        let network_info = docker
            .inspect_network::<String>(&network, None)
            .await
            .into_diagnostic()?;
        let gateway = network_info
            .ipam
            .as_ref()
            .ok_or_else(|| Report::msg("Network has no ip info"))?
            .config
            .as_deref()
            .ok_or_else(|| Report::msg("Network has no ip info"))?
            .first()
            .ok_or_else(|| Report::msg("Network has no ip info"))?
            .gateway
            .as_deref()
            .ok_or_else(|| Report::msg("Network has no ip info"))?;

        let mut containers = Vec::new();

        let sources_meta = fs::metadata(&config.sources_root).into_diagnostic()?;
        let uid = sources_meta.uid();
        let gid = sources_meta.gid();

        let mut env = vec![
            "PHP_IDE_CONFIG=serverName=haze".to_string(),
            "CHROMIUM_BIN=/usr/local/bin/chromium-no-sandbox".to_string(),
            format!("UID={}", uid),
            format!("GID={}", gid),
            format!("SQL={}", options.db.name()),
        ];
        let volumes: Vec<String> = mappings
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

        if let Some(blackfire) = config.blackfire.as_ref() {
            env.push(format!("BLACKFIRE_SERVER_ID={}", blackfire.server_id));
            env.push(format!("BLACKFIRE_SERVER_TOKEN={}", blackfire.server_token));
            env.push(format!("BLACKFIRE_CLIENT_ID={}", blackfire.client_id));
            env.push(format!("BLACKFIRE_CLIENT_TOKEN={}", blackfire.client_token));
        }

        let service_containers: Vec<Option<String>> = try_join_all(
            options
                .services
                .iter()
                .map(|service| service.spawn(docker, &id, &network, config)),
        )
        .await?;
        containers.extend(service_containers.iter().flatten().cloned());

        env.extend(
            options
                .services
                .iter()
                .flat_map(Service::env)
                .copied()
                .map(String::from),
        );

        let container = match options
            .php
            .spawn(docker, &id, env, &options.db, &network, volumes, gateway)
            .await
            .wrap_err("Failed to start php container")
        {
            Ok(container) => container,
            Err(e) => {
                for container in service_containers.iter().flatten() {
                    docker
                        .remove_container(
                            container,
                            Some(RemoveContainerOptions {
                                force: true,
                                ..RemoveContainerOptions::default()
                            }),
                        )
                        .await
                        .ok();
                }
                return Err(e);
            }
        };

        let mut tries = 0;
        let ip = loop {
            let info = docker
                .inspect_container(&container, None)
                .await
                .into_diagnostic()?;
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
                    .filter_map(|(name, network)| name.eq("haze").then_some(network))
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

        let address = config.proxy.addr(&id, ip);

        Ok(Cloud {
            id,
            network,
            containers,
            php: options.php,
            db: options.db,
            ip: Some(ip),
            workdir,
            services: options.services,
            pinned: false,
            address,
        })
    }

    pub async fn destroy(self, docker: &Docker) -> Result<()> {
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
                .into_diagnostic()
                .wrap_err("Failed to remove container")?;
        }
        docker
            .remove_network(&self.network)
            .await
            .into_diagnostic()
            .wrap_err("Failed to remove network")?;
        if self.workdir.exists() {
            if let Err(e) = remove_dir_all(self.workdir)
                .await
                .into_diagnostic()
                .wrap_err("Failed to remove work directory")
            {
                eprintln!("{}", e);
            }
        }

        Ok(())
    }

    pub async fn exec<S: Into<String>, Env: Into<String>>(
        &self,
        docker: &Docker,
        cmd: Vec<S>,
        tty: bool,
        env: Vec<Env>,
    ) -> Result<ExitCode> {
        if tty {
            exec_tty(docker, &self.id, "haze", cmd, env).await
        } else {
            exec(docker, &self.id, "haze", cmd, env, Some(stdout())).await
        }
    }

    pub async fn occ<'a, S: Into<String> + From<&'a str>, Env: Into<String>>(
        &self,
        docker: &Docker,
        mut cmd: Vec<S>,
        output: Option<&mut Vec<u8>>,
        env: Vec<Env>,
    ) -> Result<ExitCode> {
        cmd.insert(0, "occ".into());
        self.exec_with_output(docker, cmd, output, env).await
    }

    pub async fn exec_with_output<S: Into<String>, Env: Into<String>>(
        &self,
        docker: &Docker,
        cmd: Vec<S>,
        output: Option<impl Write>,
        env: Vec<Env>,
    ) -> Result<ExitCode> {
        exec(docker, &self.id, "haze", cmd, env, output).await
    }

    pub async fn list(
        docker: &Docker,
        filter: Option<String>,
        config: &HazeConfig,
    ) -> Result<Vec<Cloud>> {
        let containers = docker
            .list_containers::<String>(Some(ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await
            .into_diagnostic()?;
        let mut containers_by_id: HashMap<String, (Option<_>, Option<_>, Vec<_>)> = HashMap::new();
        for container in containers {
            let labels = container.labels.clone().unwrap_or_default();
            if let Some(cloud_id) = labels.get("haze-cloud-id") {
                if match filter.as_ref() {
                    Some(filter) => cloud_id.contains(filter),
                    None => true,
                } {
                    let entry = containers_by_id.entry(cloud_id.to_string()).or_default();
                    if labels.get("haze-type").map(String::as_str) == Some("cloud") {
                        let info = docker.inspect_container(cloud_id, None).await.ok();
                        entry.0 = Some(container);
                        entry.1 = info;
                    } else {
                        entry.2.push(container)
                    }
                }
            }
        }

        let mut sortable_containers: Vec<_> = containers_by_id
            .into_iter()
            .filter_map(|(id, (cloud, info, services))| {
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
                    .flat_map(|ty| Service::from_type(&[], ty))
                    .flatten()
                    .collect();
                let mut service_ids: Vec<String> = services
                    .iter()
                    .filter_map(|service| service.names.as_ref()?.first().map(String::clone))
                    .collect();

                let pinned = (info
                    .and_then(|info| info.host_config)
                    .and_then(|host| host.memory)
                    .unwrap()
                    % 2)
                    == 1;

                let ip = network_info.ip_address.as_ref()?.parse().ok();
                let address = if let Some(ip) = ip {
                    config.proxy.addr(&id, ip)
                } else {
                    "Not running".into()
                };

                service_ids.push(id.clone());
                Some((
                    cloud.created.unwrap_or_default(),
                    Cloud {
                        id,
                        network,
                        db,
                        php,
                        containers: service_ids,
                        ip,
                        workdir,
                        services: found_services,
                        pinned,
                        address,
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
        docker: &Docker,
        filter: Option<String>,
        config: &HazeConfig,
    ) -> Result<Cloud> {
        Cloud::list(docker, filter, config)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Report::msg("No clouds running matching filter"))
    }

    pub async fn wait_for_start(&self, docker: &Docker) -> Result<()> {
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

    pub async fn enable_app<S: Into<String>>(&self, docker: &Docker, app: S) -> Result<()> {
        self.exec(
            docker,
            vec![
                "occ".to_string(),
                "app:enable".to_string(),
                app.into(),
                "--force".to_string(),
            ],
            false,
            Vec::<String>::default(),
        )
        .await?;
        Ok(())
    }

    pub async fn pin(&self, docker: &Docker) -> Result<()> {
        // abuse memory limits as editable label
        docker
            .update_container(
                &self.id,
                UpdateContainerOptions::<String> {
                    memory: Some(PHP_MEMORY_LIMIT + 1),
                    ..UpdateContainerOptions::default()
                },
            )
            .await
            .into_diagnostic()
    }

    pub async fn unpin(&self, docker: &Docker) -> Result<()> {
        // abuse memory limits as editable label
        docker
            .update_container(
                &self.id,
                UpdateContainerOptions::<String> {
                    memory: Some(PHP_MEMORY_LIMIT),
                    ..UpdateContainerOptions::default()
                },
            )
            .await
            .into_diagnostic()
    }
}
