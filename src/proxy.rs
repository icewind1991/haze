use crate::service::ServiceTrait;
use crate::{Cloud, HazeConfig};
use crate::{Result, Service};
use bollard::Docker;
use futures_util::future::Either;
use futures_util::FutureExt;
use miette::{miette, Context, IntoDiagnostic};
use std::collections::HashMap;
use std::fs::{create_dir_all, remove_file, set_permissions};
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::UnixListener;
use tokio::signal::ctrl_c;
use tokio::spawn;
use tokio::time::sleep;
use tokio_stream::wrappers::UnixListenerStream;
use tracing::info;
use warp::host::Authority;
use warp::http::{HeaderMap, HeaderValue, Method};
use warp::hyper::body::Bytes;
use warp::path::FullPath;
use warp::Filter;
use warp_reverse_proxy::{
    extract_request_data_filter, proxy_to_and_forward_response, QueryParameters,
};

struct ActiveInstances {
    known: Mutex<HashMap<String, SocketAddr>>,
    last: Mutex<Option<SocketAddr>>,
    docker: Docker,
    config: HazeConfig,
}

impl ActiveInstances {
    pub fn new(docker: Docker, config: HazeConfig) -> Self {
        ActiveInstances {
            known: Mutex::default(),
            last: Mutex::default(),
            docker,
            config,
        }
    }

    pub async fn get(&self, name: &str) -> Option<SocketAddr> {
        if let Some(ip) = self.known.lock().unwrap().get(name).cloned() {
            return Some(ip);
        }

        let addr = if let Some(name) = name.strip_suffix("-push") {
            let cloud = Cloud::get_by_filter(&self.docker, Some(name.into()), &self.config)
                .await
                .ok()?;
            let push = cloud
                .services
                .iter()
                .filter_map(|service| match service {
                    Service::Push(push) => Some(push),
                    _ => None,
                })
                .next()?;
            let ip = push.get_ip(&self.docker, &cloud.id).await.ok()?;
            SocketAddr::new(ip, 7867)
        } else if let Some(name) = name.strip_suffix("-office") {
            let cloud = Cloud::get_by_filter(&self.docker, Some(name.into()), &self.config)
                .await
                .ok()?;
            let office = cloud
                .services
                .iter()
                .filter_map(|service| match service {
                    Service::Office(office) => Some(office),
                    _ => None,
                })
                .next()?;
            let ip = office.get_ip(&self.docker, &cloud.id).await.ok()?;
            SocketAddr::new(ip, 9980)
        } else {
            SocketAddr::new(
                Cloud::get_by_filter(&self.docker, Some(name.into()), &self.config)
                    .await
                    .ok()?
                    .ip?,
                80,
            )
        };

        println!("{name} => {addr}");

        self.known.lock().unwrap().insert(name.into(), addr);
        Some(addr)
    }

    pub fn last(&self) -> Option<SocketAddr> {
        self.last.lock().unwrap().clone()
    }

    async fn update_last(&self) {
        let last = Cloud::get_by_filter(&self.docker, None, &self.config)
            .await
            .ok()
            .and_then(|cloud| Some(SocketAddr::new(cloud.ip?, 80)));
        let mut old = self.last.lock().unwrap();
        if old.as_ref() != last.as_ref() {
            info!(instance = ?last, "Found new instance");
            *old = last;
        }
    }
}

pub async fn proxy(docker: Docker, config: HazeConfig) -> Result<()> {
    if config.proxy.listen.is_empty() {
        return Err(miette!("Proxy not configured"));
    }
    let listen = config.proxy.listen.clone();

    let base_address = config.proxy.address.clone();
    let instances = ActiveInstances::new(docker, config);
    serve(instances, listen, base_address).await
}

async fn serve(instances: ActiveInstances, listen: String, base_address: String) -> Result<()> {
    let instances = Arc::new(instances);
    let base_address = Arc::new(base_address);
    let last_instances = instances.clone();
    let instances = warp::any().map(move || instances.clone());
    let base_address = warp::any().map(move || base_address.clone());

    spawn(async move {
        loop {
            sleep(Duration::from_secs(1)).await;
            last_instances.update_last().await;
        }
    });

    let proxy = warp::any()
        .and(warp::filters::host::optional())
        .and(instances)
        .and(base_address)
        .and_then(
            move |host: Option<Authority>,
                  instances: Arc<ActiveInstances>,
                  base_address: Arc<String>| async move {
                let host = match host {
                    Some(host) => host,
                    None => return Err(warp::reject::not_found()),
                };
                let ip = if host.as_str() == base_address.as_str() {
                    instances
                        .last()
                        .ok_or_else(|| String::from("No running instance known"))
                } else {
                    let requested_instance = host.as_str().split('.').next().unwrap();
                    instances
                        .get(requested_instance)
                        .await
                        .ok_or_else(|| format!("Error {} has no known ip", requested_instance))
                };
                match ip {
                    Ok(ip) => Ok((format!("http://{}", ip), host.to_string())),
                    Err(e) => {
                        eprintln!("{}", e);
                        Err(warp::reject::not_found())
                    }
                }
            },
        )
        .untuple_one()
        .and(extract_request_data_filter())
        .and_then(
            move |proxy_address: String,
                  host: String,
                  uri: FullPath,
                  params: QueryParameters,
                  method: Method,
                  mut headers: HeaderMap,
                  body: Bytes| {
                headers.insert("host", HeaderValue::from_str(&host).unwrap());
                proxy_to_and_forward_response(
                    proxy_address,
                    String::new(),
                    uri,
                    params,
                    method,
                    headers,
                    body,
                )
            },
        );

    let cancel = async {
        ctrl_c().await.ok();
    };

    let warp_server = warp::serve(proxy);

    let server = if !listen.starts_with('/') {
        let listen = SocketAddr::from_str(&listen)
            .into_diagnostic()
            .wrap_err("Failed to parse proxy listen address")?;
        Either::Left(warp_server.bind_with_graceful_shutdown(listen, cancel).1)
    } else {
        let listen: PathBuf = listen.into();
        if let Some(parent) = listen.parent() {
            if !parent.exists() {
                create_dir_all(&parent).into_diagnostic()?;
                set_permissions(&parent, PermissionsExt::from_mode(0o755)).into_diagnostic()?;
            }
        }
        remove_file(&listen).ok();

        let listener = UnixListener::bind(&listen).into_diagnostic()?;
        set_permissions(&listen, PermissionsExt::from_mode(0o666)).into_diagnostic()?;
        let stream = UnixListenerStream::new(listener);
        Either::Right(
            warp_server
                .serve_incoming_with_graceful_shutdown(stream, cancel)
                .map(move |_| {
                    remove_file(&listen).ok();
                }),
        )
    };

    server.await;
    Ok(())
}
