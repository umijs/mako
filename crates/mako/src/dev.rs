pub(crate) mod update;
mod watch;

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::{self, Result};
use colored::Colorize;
use futures::{SinkExt, StreamExt};
use get_if_addrs::get_if_addrs;
use hyper::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Server};
use notify_debouncer_full::new_debouncer;
use tokio::sync::broadcast;
use tracing::debug;
use tungstenite::Message;
use {hyper, hyper_staticfile, hyper_tungstenite, open};

use crate::compiler::{Compiler, Context};
use crate::plugin::PluginGenerateEndParams;
use crate::utils::{process_req_url, tokio_runtime};

pub struct DevServer {
    root: PathBuf,
    compiler: Arc<Compiler>,
}

impl DevServer {
    pub fn new(root: PathBuf, compiler: Arc<Compiler>) -> Self {
        Self { root, compiler }
    }

    pub async fn serve(&self) {
        let (txws, _) = broadcast::channel::<WsMessage>(256);

        // watch
        let root = self.root.clone();
        let compiler = self.compiler.clone();
        let txws_watch = txws.clone();

        if self.compiler.context.config.dev_server.is_some() {
            std::thread::spawn(move || {
                if let Err(e) = Self::watch_for_changes(root, compiler, txws_watch) {
                    eprintln!("Error watching files: {:?}", e);
                }
            });
        } else if let Err(e) = Self::watch_for_changes(root, compiler, txws_watch) {
            eprintln!("Error watching files: {:?}", e);
        }

        // server
        if self.compiler.context.config.dev_server.is_some() {
            let config_port = self
                .compiler
                .context
                .config
                .dev_server
                .as_ref()
                .unwrap()
                .port;
            let port = Self::find_available_port("127.0.0.1".to_string(), config_port);
            let addr: SocketAddr = ([127, 0, 0, 1], port).into();
            let context = self.compiler.context.clone();
            let txws = txws.clone();
            let make_svc = make_service_fn(move |_conn| {
                let context = context.clone();
                let txws = txws.clone();
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |req| {
                        let context = context.clone();
                        let txws = txws.clone();
                        let staticfile = {
                            let mut sf =
                                hyper_staticfile::Static::new(context.config.output.path.clone());
                            sf.cache_headers(Some(0));
                            sf
                        };
                        async move { Self::handle_requests(req, context, staticfile, txws).await }
                    }))
                }
            });
            let server = Server::bind(&addr).serve(make_svc);
            // TODO: print when mako is run standalone
            if std::env::var("MAKO_CLI").is_ok() {
                println!();
                if config_port != port {
                    println!(
                        "{}",
                        format!("Port {} is in use, using {} instead.", config_port, port)
                            .to_string()
                            .yellow(),
                    );
                }
                println!(
                    "Local:   {}",
                    format!("http://localhost:{}/", port).to_string().cyan()
                );
                let ips = Self::get_ips();
                let ips = ips
                    .iter()
                    .filter(|ip| !ip.starts_with("127."))
                    .collect::<Vec<_>>();
                for ip in ips {
                    println!(
                        "Network: {}",
                        format!("http://{}:{}/", ip, port).to_string().cyan()
                    );
                }
                println!();
                open::that(format!("http://localhost:{}/", port)).unwrap();
            }
            debug!("Listening on http://{:?}", addr);
            if let Err(e) = server.await {
                eprintln!("Error starting server: {:?}", e);
            }
        }
    }

    async fn handle_requests(
        req: Request<Body>,
        context: Arc<Context>,
        staticfile: hyper_staticfile::Static,
        txws: broadcast::Sender<WsMessage>,
    ) -> Result<hyper::Response<Body>> {
        debug!("> {} {}", req.method().to_string(), req.uri().path());

        let mut path = req.uri().path().to_string();
        let public_path = &context.config.public_path;
        if !public_path.is_empty() && public_path.starts_with('/') && public_path != "/" {
            path = match process_req_url(public_path, &path) {
                Ok(p) => p,
                Err(_) => {
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::BAD_REQUEST)
                        .body(hyper::Body::from("Bad Request"))
                        .unwrap());
                }
            };
        }
        let path_without_slash_start = path.trim_start_matches('/');
        let not_found_response = || {
            hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(hyper::Body::empty())
                .unwrap()
        };
        match path.as_str() {
            "/__/hmr-ws" => {
                if hyper_tungstenite::is_upgrade_request(&req) {
                    debug!("new websocket connection");
                    let (response, websocket) = hyper_tungstenite::upgrade(req, None).unwrap();
                    let txws = txws.clone();
                    tokio_runtime::spawn(async move {
                        let receiver = txws.subscribe();
                        Self::handle_websocket(websocket, receiver).await.unwrap();
                    });
                    Ok(response)
                } else {
                    Ok(not_found_response())
                }
            }
            _ => {
                // for bundle outputs

                let ext = path.rsplit('.').next();
                let content_type = match ext {
                    None => "text/plain; charset=utf-8",
                    Some("js") => "application/javascript; charset=utf-8",
                    Some("css") => "text/css; charset=utf-8",
                    Some("map") | Some("json") => "application/json; charset=utf-8",
                    Some(_) => "text/plain; charset=utf-8",
                };

                // staticfile has 302 problems when modify tooooo fast in 1 second
                // it will response 302 and we will get the old file
                // TODO: fix the 302 problem?
                if !context.config.write_to_disk {
                    if let Some(res) = context.get_static_content(path_without_slash_start) {
                        debug!("serve with context.get_static_content: {}", path);

                        return Ok(hyper::Response::builder()
                            .status(hyper::StatusCode::OK)
                            .header(CACHE_CONTROL, "no-cache")
                            .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                            .header(CONTENT_TYPE, content_type)
                            .body(hyper::Body::from(res))
                            .unwrap());
                    }
                }
                // for cached dep
                let abs_path = context
                    .root
                    .join("node_modules/.cache_mako/chunks")
                    .join(path_without_slash_start);
                if !path_without_slash_start.is_empty() && abs_path.exists() {
                    return std::fs::read(abs_path).map_or(Ok(not_found_response()), |bytes| {
                        Ok(hyper::Response::builder()
                            .status(hyper::StatusCode::OK)
                            .header(CONTENT_TYPE, content_type)
                            .header(CACHE_CONTROL, "no-cache")
                            .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                            .body(hyper::Body::from(bytes))
                            .unwrap())
                    });
                }

                // for hmr files
                debug!("< static file serve: {}", path);
                let req = hyper::Request::builder()
                    .uri(path)
                    .body(hyper::Body::empty())
                    .unwrap();
                let res = staticfile.serve(req).await;
                res.map_err(anyhow::Error::from)
            }
        }
    }

    fn get_ips() -> Vec<String> {
        let mut ips = vec![];
        match get_if_addrs() {
            Ok(if_addrs) => {
                for if_addr in if_addrs {
                    if let get_if_addrs::IfAddr::V4(addr) = if_addr.addr {
                        let ip = addr.ip.to_string();
                        ips.push(ip);
                    }
                }
            }
            Err(_e) => {}
        }
        ips
    }

    fn find_available_port(host: String, port: u16) -> u16 {
        let mut port = port;
        if TcpListener::bind((host.clone(), port)).is_ok() {
            port
        } else {
            port += 1;
            Self::find_available_port(host, port)
        }
    }

    // TODO: refact socket message data structure
    async fn handle_websocket(
        websocket: hyper_tungstenite::HyperWebsocket,
        mut receiver: broadcast::Receiver<WsMessage>,
    ) -> Result<()> {
        let websocket = websocket.await?;
        let (mut sender, mut ws_recv) = websocket.split();
        let task = tokio_runtime::spawn(async move {
            loop {
                if let Ok(msg) = receiver.recv().await {
                    if sender
                        .send(Message::text(format!(r#"{{"hash":"{}"}}"#, msg.hash)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
        while let Some(message) = ws_recv.next().await {
            if let Ok(Message::Close(_)) = message {
                break;
            }
        }
        debug!("websocket connection disconnected");
        task.abort();
        Ok(())
    }

    fn watch_for_changes(
        root: PathBuf,
        compiler: Arc<Compiler>,
        txws: broadcast::Sender<WsMessage>,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        // let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
        let mut debouncer = new_debouncer(Duration::from_millis(10), None, tx).unwrap();
        let mut watcher = watch::Watcher::new(&root, debouncer.watcher(), &compiler);
        watcher.watch()?;

        let initial_hash = compiler.full_hash();
        let mut snapshot_hash = Box::new(initial_hash);
        let mut hmr_hash = Box::new(initial_hash);

        for result in rx {
            if result.is_err() {
                eprintln!("Error watching files: {:?}", result.err().unwrap());
                continue;
            }
            let paths = watch::Watcher::normalize_events(result.unwrap());
            if !paths.is_empty() {
                let compiler = compiler.clone();
                let txws = txws.clone();
                if let Err(e) =
                    Self::rebuild(paths, compiler, txws, &mut snapshot_hash, &mut hmr_hash)
                {
                    eprintln!("Error rebuilding: {:?}", e);
                }
            }
        }
        Ok(())
    }

    fn rebuild(
        paths: Vec<PathBuf>,
        compiler: Arc<Compiler>,
        txws: broadcast::Sender<WsMessage>,
        last_snapshot_hash: &mut Box<u64>,
        hmr_hash: &mut Box<u64>,
    ) -> Result<()> {
        debug!("watch paths detected: {:?}", paths);
        debug!("checking update status...");
        println!("Checking...");
        let update_result = compiler.update(paths);
        let has_missing_deps = {
            compiler
                .context
                .modules_with_missing_deps
                .read()
                .unwrap()
                .len()
                > 0
        };
        debug!("has_missing_deps: {}", has_missing_deps);
        debug!("checking update status... done");

        if let Err(e) = update_result {
            debug!("checking update status... failed");
            eprintln!("{}", e);
            // do not return error, since it's already printed
            return Ok(());
        }

        let res = update_result.unwrap();
        let is_updated = res.is_updated();
        debug!("update status is ok, is_updated: {}", is_updated);
        if !is_updated {
            println!("No changes");
            return Ok(());
        }

        let t_compiler = Instant::now();
        let start_time = chrono::Local::now().timestamp_millis();
        let next_hash = compiler.generate_hot_update_chunks(res, **last_snapshot_hash, **hmr_hash);
        debug!(
            "hot update chunks generated, next_full_hash: {:?}",
            next_hash
        );
        if let Err(e) = next_hash {
            eprintln!("Error in watch: {:?}", e);
            return Err(e);
        }
        let (next_snapshot_hash, next_hmr_hash, current_hmr_hash) = next_hash.unwrap();
        debug!(
            "hash info, next: {:?}, last: {:?}, is_equal: {}",
            next_snapshot_hash,
            last_snapshot_hash,
            next_snapshot_hash == **last_snapshot_hash
        );
        if next_snapshot_hash == **last_snapshot_hash {
            debug!("hash equals, will not do full rebuild");
            return Ok(());
        } else {
            **last_snapshot_hash = next_snapshot_hash;
            **hmr_hash = next_hmr_hash;
        }

        debug!("full rebuild...");

        compiler.context.stats_info.clear_assets();

        let mut stats = compiler
            .emit_dev_chunks(next_hmr_hash, current_hmr_hash)
            .map_err(|e| {
                debug!("  > build failed: {:?}", e);
                e
            })?;

        stats.start_time = start_time;
        stats.end_time = chrono::Local::now().timestamp_millis();

        debug!("full rebuild...done");
        if !has_missing_deps {
            println!(
                "Full rebuilt in {}",
                format!("{}ms", t_compiler.elapsed().as_millis()).bold()
            );
            let params = PluginGenerateEndParams {
                is_first_compile: false,
                time: t_compiler.elapsed().as_millis() as i64,
                stats,
            };
            compiler
                .context
                .plugin_driver
                .generate_end(&params, &compiler.context)
                .map_err(|e| {
                    debug!("generate end failed: {:?}", e);
                    e
                })?;
            compiler
                .context
                .plugin_driver
                .write_bundle(&compiler.context)
                .map_err(|e| {
                    debug!("write bundle failed: {:?}", e);
                    e
                })?;
        }

        let receiver_count = txws.receiver_count();
        debug!("receiver count: {}", receiver_count);
        if receiver_count > 0 {
            txws.send(WsMessage { hash: **hmr_hash }).unwrap();
            debug!("send message to clients");
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct WsMessage {
    hash: u64,
}
