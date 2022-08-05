use crate::Result;
use crate::{Cloud, HazeConfig};
use bollard::Docker;
use futures_util::future::Either;
use futures_util::FutureExt;
use miette::{miette, Context, IntoDiagnostic};
use std::collections::HashMap;
use std::fs::{create_dir_all, remove_file, set_permissions};
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::net::UnixListener;
use tokio::signal::ctrl_c;
use tokio_stream::wrappers::UnixListenerStream;
use warp::host::Authority;
use warp::http::{HeaderMap, HeaderValue, Method};
use warp::hyper::body::Bytes;
use warp::path::FullPath;
use warp::Filter;
use warp_reverse_proxy::{
    extract_request_data_filter, proxy_to_and_forward_response, QueryParameters,
};

struct ActiveInstances {
    known: Mutex<HashMap<String, IpAddr>>,
    docker: Docker,
    config: HazeConfig,
}

impl ActiveInstances {
    pub fn new(docker: Docker, config: HazeConfig) -> Self {
        ActiveInstances {
            known: Mutex::default(),
            docker,
            config,
        }
    }

    pub async fn get(&self, name: &str) -> Option<IpAddr> {
        if let Some(ip) = self.known.lock().unwrap().get(name).cloned() {
            return Some(ip);
        }

        let cloud = Cloud::get_by_filter(&self.docker, Some(name.into()), &self.config)
            .await
            .ok()?;

        if let Some(ip) = cloud.ip {
            println!("{name} => {ip}");

            self.known.lock().unwrap().insert(name.into(), ip);
            return Some(ip);
        }

        None
    }
}

pub async fn proxy(docker: Docker, config: HazeConfig) -> Result<()> {
    if config.proxy.listen.is_empty() {
        return Err(miette!("Proxy not configured"));
    }
    let listen = config.proxy.listen.clone();

    let instances = ActiveInstances::new(docker, config);
    serve(instances, listen).await
}

async fn serve(instances: ActiveInstances, listen: String) -> Result<()> {
    let instances = Arc::new(instances);
    let instances = warp::any().map(move || instances.clone());

    let proxy = warp::any()
        .and(warp::filters::host::optional())
        .and(instances)
        .and_then(
            move |host: Option<Authority>, instances: Arc<ActiveInstances>| async move {
                let host = match host {
                    Some(host) => host,
                    None => return Err(warp::reject::not_found()),
                };
                let requested_instance = host.as_str().split('.').next().unwrap();
                if let Some(ip) = instances.get(requested_instance).await {
                    Ok((format!("http://{}", ip), host.to_string()))
                } else {
                    eprintln!("Error {} has no known ip", requested_instance);
                    Err(warp::reject::not_found())
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