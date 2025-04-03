use crate::service::ServiceTrait;
use crate::Result;
use crate::{Cloud, HazeConfig};
use axum::http::header::HOST;
use axum::http::HeaderValue;
use axum::{
    body::Body,
    extract::{Request, State},
    response::{IntoResponse, Response},
    Router,
};
use bollard::Docker;
use hyper::StatusCode;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use miette::{miette, IntoDiagnostic};
use std::collections::HashMap;
use std::fs::{create_dir_all, set_permissions};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::UnixListener;
use tokio::signal::ctrl_c;
use tokio::spawn;
use tokio::time::sleep;
use tracing::{debug, error, info};

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

        // service proxy
        let addr = if name.matches('-').count() == 2 {
            let (name, service_name) = name.rsplit_once('-').unwrap();
            let cloud = Cloud::get_by_filter(&self.docker, Some(name.into()), &self.config)
                .await
                .ok()?;
            let service = cloud
                .services()
                .find(|service| service.name() == service_name)?;
            let ip = service.get_ip(&self.docker, &cloud.id).await.ok()??;
            SocketAddr::new(ip, service.proxy_port())
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
        *self.last.lock().unwrap()
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

#[derive(Clone)]
struct AppState {
    instances: Arc<ActiveInstances>,
    base_address: Arc<String>,
    proxy_client: Arc<Client>,
}

async fn serve(instances: ActiveInstances, listen: String, base_address: String) -> Result<()> {
    let instances = Arc::new(instances);
    let base_address = Arc::new(base_address);
    let last_instances = instances.clone();

    let proxy_client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());

    spawn(async move {
        loop {
            sleep(Duration::from_secs(1)).await;
            last_instances.update_last().await;
        }
    });

    let cancel = async {
        ctrl_c().await.ok();
    };

    let app = Router::new().fallback(handler).with_state(AppState {
        instances: instances.clone(),
        base_address: base_address.clone(),
        proxy_client: Arc::new(proxy_client),
    });

    if !listen.starts_with('/') {
        let addr: SocketAddr = listen.parse().into_diagnostic()?;
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        println!("listening on {}", listener.local_addr().unwrap());
        axum::serve(listener, app)
            .with_graceful_shutdown(cancel)
            .await
            .unwrap();
    } else {
        let listen: PathBuf = listen.into();
        if let Some(parent) = listen.parent() {
            if !parent.exists() {
                create_dir_all(parent).into_diagnostic()?;
                set_permissions(parent, PermissionsExt::from_mode(0o755)).into_diagnostic()?;
            }
        }
        let _ = tokio::fs::remove_file(&listen).await;

        let uds = UnixListener::bind(&listen).unwrap();
        set_permissions(&listen, PermissionsExt::from_mode(0o666)).into_diagnostic()?;

        axum::serve(uds, app)
            .with_graceful_shutdown(cancel)
            .await
            .unwrap();
    }

    Ok(())
}

async fn get_remote(
    host: Option<&HeaderValue>,
    instances: &ActiveInstances,
    base_address: &str,
) -> Result<SocketAddr, String> {
    let host = match host.and_then(|host| host.to_str().ok()) {
        Some(host) => host,
        None => return Err("No or invalid hostname provided".into()),
    };
    let ip = if host == base_address {
        instances
            .last()
            .ok_or_else(|| String::from("No running instance known"))
    } else {
        let requested_instance = host.split('.').next().unwrap();
        instances
            .get(requested_instance)
            .await
            .ok_or_else(|| format!("Error {} has no known ip", requested_instance))
    };
    match ip {
        Ok(ip) => Ok(ip),
        Err(e) => {
            eprintln!("{}", e);
            Err(e)
        }
    }
}

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

async fn handler(State(state): State<AppState>, mut req: Request) -> Result<Response, StatusCode> {
    let host = req.headers().get(HOST).cloned();
    let remote = match get_remote(host.as_ref(), &state.instances, &state.base_address).await {
        Ok(remote) => remote,
        Err(e) => {
            return Ok(hyper::Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(e.into())
                .unwrap())
        }
    };

    let uri = format!("http://{remote}");
    debug!(target = uri, "proxying request");

    // fix weird duplicate host header
    req.headers_mut().remove(HOST);
    if let Some(host) = host {
        req.headers_mut().insert(HOST, host.clone());
    }

    match hyper_reverse_proxy::call(
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        &uri,
        req,
        &state.proxy_client,
    )
    .await
    {
        Ok(response) => Ok(response.map(Body::new)),
        Err(error) => {
            error!(%error, "error while proxying request");
            Ok(StatusCode::BAD_REQUEST.into_response())
        }
    }
}
