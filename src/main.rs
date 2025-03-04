extern crate core;

use crate::args::{ExecService, HazeArgs};
use crate::cloud::{Cloud, CloudOptions};
use crate::config::HazeConfig;
use crate::database::DatabaseFamily;
use crate::exec::container_logs;
use crate::git::checkout_all;
use crate::network::clear_networks;
use crate::proxy::proxy;
use crate::service::Service;
use crate::service::ServiceTrait;
use bollard::Docker;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use std::env::vars;
use std::io::stdout;
use std::os::unix::process::CommandExt;
use std::process::Command;

mod args;
mod cloud;
mod config;
mod database;
mod exec;
mod git;
mod image;
mod mapping;
mod network;
mod php;
mod proxy;
mod service;

static FORWARD_ENV: &[&str] = &[
    "OCC_LOG",
    "OC_PASS",
    "XDEBUG_MODE",
    "XDEBUG_TRIGGER",
    "XDEBUG_CONFIG",
];

fn get_forward_env() -> Vec<String> {
    vars()
        .filter(|(var, _)| FORWARD_ENV.contains(&var.as_str()))
        .map(|(var, value)| format!("{var}={value}"))
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    miette::set_panic_hook();
    tracing_subscriber::fmt::init();

    let docker = Docker::connect_with_local_defaults()
        .into_diagnostic()
        .wrap_err("Failed to connect to docker")?;
    let config = HazeConfig::load().wrap_err("Failed to load config")?;

    let args = HazeArgs::parse(&config.preset, std::env::args())?;

    match args {
        HazeArgs::Clean => {
            let list = Cloud::list(&docker, None, &config).await?;
            for cloud in list.into_iter().filter(|cloud| !cloud.pinned) {
                if let Err(e) = cloud.destroy(&docker).await {
                    eprintln!("Error while removing cloud: {:#}", e);
                }
            }
            clear_networks(&docker).await?;
        }
        HazeArgs::List { filter } => {
            let list = Cloud::list(&docker, filter, &config).await?;
            for cloud in list {
                let mut services: Vec<_> = cloud.services().map(Service::name).collect();
                services.push(cloud.db().name());
                let services = services.join(", ");
                let pin = if cloud.pinned { "*" } else { "" };
                println!(
                    "Cloud {}{}, {}, {}, running on {}",
                    cloud.id,
                    pin,
                    cloud.php().name(),
                    services,
                    cloud.address
                );
            }
        }
        HazeArgs::Start { options } => {
            setup(&docker, options, &config).await?;
        }
        HazeArgs::Stop { filter } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            cloud.destroy(&docker).await?;
        }
        HazeArgs::Logs {
            filter,
            follow,
            count,
            service,
        } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            let container = if let Some(service) = service {
                service
                    .container_name(&cloud.id)
                    .ok_or_else(|| Report::msg("service has no logs".to_string()))?
            } else {
                cloud.id
            };
            container_logs(&docker, stdout(), &container, count.unwrap_or(20), follow).await?;
        }
        HazeArgs::Exec {
            filter,
            service,
            command,
        } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            match service {
                None => {
                    cloud
                        .exec(
                            &docker,
                            if command.is_empty() {
                                vec!["bash".to_string()]
                            } else {
                                command
                            },
                            atty::is(atty::Stream::Stdout),
                            get_forward_env(),
                        )
                        .await?;
                }
                Some(ExecService::Db) => {
                    cloud
                        .db()
                        .exec_sh(
                            &docker,
                            &cloud.id,
                            if command.is_empty() {
                                vec!["bash".to_string()]
                            } else {
                                command
                            },
                            atty::is(atty::Stream::Stdout),
                        )
                        .await?;
                }
            }
        }
        HazeArgs::Occ {
            filter,
            mut command,
        } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            command.insert(0, "occ".to_string());
            cloud
                .exec(
                    &docker,
                    command,
                    atty::is(atty::Stream::Stdout),
                    get_forward_env(),
                )
                .await?;
        }
        HazeArgs::Db { filter, root } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            cloud.db().exec(&docker, &cloud.id, root).await?;
        }
        HazeArgs::Open { filter } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            match cloud.ip {
                Some(_) => opener::open(cloud.address).into_diagnostic()?,
                None => eprintln!("{} is not running", cloud.id),
            }
        }
        HazeArgs::Test { options, mut args } => {
            let cloud = Cloud::create(&docker, options, &config).await?;
            println!("Waiting for servers to start");
            cloud.wait_for_start(&docker).await?;

            if !cloud.preset_config.is_empty() {
                println!("Writing preset config");
                let encoded_preset_config =
                    serde_json::to_string(&cloud.preset_config).into_diagnostic()?;
                cloud
                    .write_file(&docker, "config/preset.config.json", encoded_preset_config)
                    .await?;
                cloud.write_file(&docker, "config/preset.config.php", "<?php $CONFIG=json_decode(file_get_contents(__DIR__ . '/preset.config.json'), true);").await?;
            }

            println!("Installing");
            if let Err(e) = cloud
                .exec(
                    &docker,
                    vec![
                        "install",
                        &config.auto_setup.username,
                        &config.auto_setup.password,
                    ],
                    false,
                    Vec::<String>::default(),
                )
                .await
            {
                cloud.destroy(&docker).await?;
                return Err(e);
            }
            if let Some(app) = args
                .first()
                .as_ref()
                .and_then(|path| path.strip_prefix("apps/"))
                .map(|path| &path[0..path.find('/').unwrap_or(path.len())])
            {
                if app.starts_with("files_") {
                    cloud.enable_app(&docker, "files_external").await?;
                }
                println!("Enabling {}", app);
                cloud.enable_app(&docker, app).await?;
            }
            args.insert(0, "tests".to_string());
            cloud.exec(&docker, args, false, get_forward_env()).await?;
            cloud.destroy(&docker).await?;
        }
        HazeArgs::Integration { options, mut args } => {
            let cloud = Cloud::create(&docker, options, &config).await?;
            println!("Waiting for servers to start");
            cloud.wait_for_start(&docker).await?;
            println!("Installing");
            if let Err(e) = cloud
                .exec(
                    &docker,
                    vec![
                        "install",
                        &config.auto_setup.username,
                        &config.auto_setup.password,
                    ],
                    false,
                    Vec::<String>::default(),
                )
                .await
            {
                cloud.destroy(&docker).await?;
                return Err(e);
            }
            args.insert(0, "integration".to_string());
            cloud.exec(&docker, args, false, get_forward_env()).await?;
            cloud.destroy(&docker).await?;
        }
        HazeArgs::Fmt { path } => {
            let cloud = Cloud::create(&docker, CloudOptions::default(), &config).await?;
            let mut out_buffer = Vec::<u8>::with_capacity(1024);
            println!("Waiting for servers to start");
            cloud.wait_for_start(&docker).await?;
            println!("Installing composer");
            if let Err(e) = cloud
                .exec_with_output(
                    &docker,
                    vec!["composer", "install"],
                    Some(&mut out_buffer),
                    Vec::<String>::default(),
                )
                .await
                .and_then(|c| c.to_result())
            {
                eprintln!("{}", String::from_utf8_lossy(&out_buffer));
                cloud.destroy(&docker).await?;
                return Err(e);
            }
            out_buffer.clear();
            println!("Formatting");
            if let Err(e) = cloud
                .exec(
                    &docker,
                    vec!["composer", "run", "cs:fix", path.as_str()],
                    false,
                    Vec::<String>::default(),
                )
                .await
            {
                cloud.destroy(&docker).await?;
                return Err(e);
            }
            println!("Cleanup");
            if let Err(e) = cloud
                .exec_with_output(
                    &docker,
                    vec!["git", "clean", "-fd", "lib/composer"],
                    Some(&mut out_buffer),
                    Vec::<String>::default(),
                )
                .await
                .and_then(|c| c.to_result())
            {
                eprintln!("{}", String::from_utf8_lossy(&out_buffer));
                cloud.destroy(&docker).await?;
                return Err(e);
            }
            if let Err(e) = cloud
                .exec_with_output(
                    &docker,
                    vec!["git", "checkout", "lib/composer"],
                    Some(&mut out_buffer),
                    Vec::<String>::default(),
                )
                .await
                .and_then(|c| c.to_result())
            {
                eprintln!("{}", String::from_utf8_lossy(&out_buffer));
                cloud.destroy(&docker).await?;
                return Err(e);
            }
            cloud.destroy(&docker).await?;
        }
        HazeArgs::Shell { command, options } => {
            let cloud = setup(&docker, options, &config).await?;
            cloud
                .exec(
                    &docker,
                    if command.is_empty() {
                        vec!["bash".to_string()]
                    } else {
                        command
                    },
                    true,
                    get_forward_env(),
                )
                .await?;
            cloud.destroy(&docker).await?;
        }
        HazeArgs::Pin { filter } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            cloud.pin(&docker).await?;
        }
        HazeArgs::Unpin { filter } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            cloud.unpin(&docker).await?;
        }
        HazeArgs::Proxy => {
            proxy(docker, config).await?;
        }
        HazeArgs::Checkout { branch } => {
            checkout_all(&config.sources_root, &branch)?;
        }
        HazeArgs::Env {
            filter,
            command,
            args,
        } => {
            let cloud = Cloud::get_by_filter(&docker, filter, &config).await?;
            let ip = cloud
                .ip
                .ok_or_else(|| Report::msg(format!("{} is not running", cloud.id)))?;
            let db_type = match cloud.db().family() {
                DatabaseFamily::Sqlite => {
                    return Err(Report::msg("sqlite is not supported with `haze env`"));
                }
                DatabaseFamily::Oracle => {
                    return Err(Report::msg("oracle is not supported with `haze env`"));
                }
                DatabaseFamily::Mysql | DatabaseFamily::MariaDB => "mysql",
                DatabaseFamily::Postgres => "postgresql",
            };
            let db_ip = cloud
                .db()
                .ip(&docker, &cloud.id)
                .await
                .ok_or_else(|| Report::msg(format!("{}-db is not running", cloud.id)))?;

            let err = Command::new(command)
                .args(args)
                .env("REDIS_URL", format!("redis://{}", ip))
                .env("NEXTCLOUD_URL", &cloud.address)
                .env(
                    "DATABASE_URL",
                    format!("{}://haze:haze@{}/haze", db_type, db_ip),
                )
                .exec();
            return Err(err).into_diagnostic();
        }
    };

    Ok(())
}

async fn setup(docker: &Docker, options: CloudOptions, config: &HazeConfig) -> Result<Cloud> {
    let cloud = Cloud::create(docker, options, config).await?;
    println!("{}", cloud.address);
    let host = cloud.address.split_once("://").expect("no address?").1;
    if config.auto_setup.enabled {
        println!("Waiting for servers to start");
        cloud.wait_for_start(docker).await?;

        if !cloud.preset_config.is_empty() {
            println!("Writing preset config");
            let encoded_preset_config =
                serde_json::to_string(&cloud.preset_config).into_diagnostic()?;
            cloud
                .write_file(docker, "config/preset.config.json", encoded_preset_config)
                .await?;
            cloud.write_file(docker, "config/preset.config.php", "<?php $CONFIG=json_decode(file_get_contents(__DIR__ . '/preset.config.json'), true);").await?;
        }

        println!(
            "Installing with username {} and password {}",
            config.auto_setup.username, config.auto_setup.password
        );
        let ip_str = format!("{}", cloud.ip.unwrap());
        cloud
            .exec(
                docker,
                vec![
                    "install",
                    &config.auto_setup.username,
                    &config.auto_setup.password,
                ],
                false,
                Vec::<String>::default(),
            )
            .await?;
        cloud
            .occ(
                docker,
                vec![
                    "config:system:set",
                    "overwrite.cli.url",
                    "--value",
                    &cloud.address,
                ],
                None,
                Vec::<String>::default(),
            )
            .await?;
        cloud
            .occ(
                docker,
                vec!["config:system:set", "overwritehost", "--value", host],
                None,
                Vec::<String>::default(),
            )
            .await?;
        if cloud.address.contains("https://") {
            cloud
                .occ(
                    docker,
                    vec!["config:system:set", "overwriteprotocol", "--value", "https"],
                    None,
                    Vec::<String>::default(),
                )
                .await?;
        }

        let domains = [ip_str.as_str(), "cloud", &cloud.id, host];
        for (i, domain) in domains.iter().enumerate() {
            cloud
                .occ(
                    docker,
                    vec![
                        "config:system:set",
                        "trusted_domains",
                        &format!("{}", i),
                        "--value",
                        domain,
                    ],
                    None,
                    Vec::<String>::default(),
                )
                .await?;
        }

        for service in cloud.services() {
            for app in service.apps() {
                cloud
                    .exec(
                        docker,
                        vec!["occ", "app:enable", *app, "--force"],
                        false,
                        Vec::<String>::default(),
                    )
                    .await?;
            }
        }
        for service in cloud.services() {
            for cmd in service.post_setup(docker, &cloud.id, config).await? {
                cloud
                    .exec(
                        docker,
                        shell_words::split(&cmd).into_diagnostic()?,
                        false,
                        Vec::<String>::default(),
                    )
                    .await?;
            }
        }
        for cmd in &config.auto_setup.post_setup {
            cloud
                .exec(
                    docker,
                    shell_words::split(cmd).into_diagnostic()?,
                    false,
                    Vec::<String>::default(),
                )
                .await?;
        }
    }
    Ok(cloud)
}
