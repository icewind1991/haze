use crate::service::ServiceTrait;
use crate::Result;
use crate::{Cloud, HazeConfig};
use bollard::Docker;
use miette::{miette, IntoDiagnostic};
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs::{create_dir_all, remove_file, set_permissions};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::ctrl_c;
use tokio::spawn;
use tokio::time::sleep;
use tokio_stream::wrappers::UnixListenerStream;
use tracing::info;
use warp::http::header::HOST;
use warp::http::HeaderValue;
use warp::hyper::server::accept::from_stream;
use warp::hyper::server::conn::AddrStream;
use warp::hyper::service::{make_service_fn, service_fn};
use warp::hyper::{Body, Request, Response, Server, StatusCode};

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

async fn serve(instances: ActiveInstances, listen: String, base_address: String) -> Result<()> {
    let instances = Arc::new(instances);
    let base_address = Arc::new(base_address);
    let last_instances = instances.clone();

    spawn(async move {
        loop {
            sleep(Duration::from_secs(1)).await;
            last_instances.update_last().await;
        }
    });

    let cancel = async {
        ctrl_c().await.ok();
    };

    let handler = move |remote_addr| {
        let instances = instances.clone();
        let base_address = base_address.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle(remote_addr, req, instances.clone(), base_address.clone())
            }))
        }
    };

    if !listen.starts_with('/') {
        let make_svc = make_service_fn(|conn: &AddrStream| handler(conn.remote_addr().ip()));
        let addr: SocketAddr = listen.parse().into_diagnostic()?;
        Server::bind(&addr)
            .serve(make_svc)
            .with_graceful_shutdown(cancel)
            .await
            .into_diagnostic()?;
    } else {
        let make_svc =
            make_service_fn(move |_conn: &UnixStream| handler(Ipv4Addr::UNSPECIFIED.into()));
        let listen: PathBuf = listen.into();
        if let Some(parent) = listen.parent() {
            if !parent.exists() {
                create_dir_all(parent).into_diagnostic()?;
                set_permissions(parent, PermissionsExt::from_mode(0o755)).into_diagnostic()?;
            }
        }
        remove_file(&listen).ok();

        let listener = UnixListener::bind(&listen).into_diagnostic()?;
        set_permissions(&listen, PermissionsExt::from_mode(0o666)).into_diagnostic()?;
        let stream = UnixListenerStream::new(listener);
        let acceptor = from_stream(stream);
        Server::builder(acceptor)
            .serve(make_svc)
            .with_graceful_shutdown(cancel)
            .await
            .into_diagnostic()?;
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

async fn handle(
    client_ip: IpAddr,
    req: Request<Body>,
    instances: Arc<ActiveInstances>,
    base_address: Arc<String>,
) -> Result<Response<Body>, Infallible> {
    let host = req.headers().get(HOST);
    let remote = match get_remote(host, &instances, &base_address).await {
        Ok(remote) => remote,
        Err(e) => {
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(e.into())
                .unwrap())
        }
    };

    let forward = format!("http://{}", remote);
    let client = hyper::Client::builder().build_http();
    match hyper_reverse_proxy::call(client_ip, &forward, req, &client).await {
        Ok(response) => Ok(response),
        Err(_error) => Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()),
    }
}
