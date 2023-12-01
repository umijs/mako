use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, UNIX_EPOCH};

use mako_core::anyhow::{self, Result};
use mako_core::colored::Colorize;
use mako_core::futures::{SinkExt, StreamExt};
use mako_core::hyper::header::CONTENT_TYPE;
use mako_core::hyper::http::HeaderValue;
use mako_core::hyper::server::conn::AddrIncoming;
use mako_core::hyper::server::Builder;
use mako_core::hyper::service::{make_service_fn, service_fn};
use mako_core::hyper::{Body, Request, Server};
use mako_core::notify::{self, RecommendedWatcher, Watcher};
use mako_core::notify_debouncer_full::new_debouncer;
use mako_core::tokio::sync::broadcast;
use mako_core::tracing::debug;
use mako_core::tungstenite::Message;
use mako_core::{hyper, hyper_staticfile, hyper_tungstenite, tokio};

use crate::compiler::{Compiler, Context};
use crate::watch::{Watch, WatchEvent};

pub struct DevServer {
    root: PathBuf,
    compiler: Arc<Compiler>,
}

impl DevServer {
    pub fn new(root: PathBuf, compiler: Arc<Compiler>) -> Self {
        Self { root, compiler }
    }

    pub async fn serve(
        &self,
        callback: impl Fn(OnDevCompleteParams) + Send + Sync + Clone + 'static,
    ) {
        let (txws, _) = broadcast::channel::<WsMessage>(256);

        // watch
        let root = self.root.clone();
        let compiler = self.compiler.clone();
        let txws_watch = txws.clone();
        std::thread::spawn(move || {
            if let Err(e) = Self::watch_for_changes(root, compiler, txws_watch, callback) {
                println!("Error watching files: {:?}", e);
            }
        });

        // server
        let port = self
            .compiler
            .context
            .config
            .hmr_port
            .parse::<u16>()
            .unwrap();
        // TODO: host
        // let host = self.compiler.context.config.hmr_host.clone();
        // TODO: find free port
        let addr = ([127, 0, 0, 1], port);
        let context = self.compiler.context.clone();
        let txws = txws.clone();
        let make_svc = make_service_fn(move |_conn| {
            let context = context.clone();
            let txws = txws.clone();
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    let context = context.clone();
                    let txws = txws.clone();
                    let staticfile =
                        hyper_staticfile::Static::new(context.config.output.path.clone());
                    async move { Self::handle_requests(req, context, staticfile, txws).await }
                }))
            }
        });
        let server = Server::bind(&addr.into()).serve(make_svc);
        println!("Listening on http://{:?}", addr);
        if let Err(e) = server.await {
            println!("Error starting server: {:?}", e);
        }
    }

    async fn handle_requests(
        req: Request<Body>,
        context: Arc<Context>,
        staticfile: hyper_staticfile::Static,
        txws: broadcast::Sender<WsMessage>,
    ) -> Result<hyper::Response<Body>> {
        let path = req.uri().path();
        let path_without_slash_start = path.trim_start_matches('/');
        let not_found_response = || {
            hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(hyper::Body::empty())
                .unwrap()
        };
        match path {
            "/__/hmr-ws" => {
                if hyper_tungstenite::is_upgrade_request(&req) {
                    println!(">>> upgrade request");
                    let (response, websocket) = hyper_tungstenite::upgrade(req, None).unwrap();
                    let txws = txws.clone();
                    tokio::spawn(async move {
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
                // staticfile has 302 problems when modify tooooo fast in 1 second
                // it will response 302 and we will get the old file
                // TODO: fix the 302 problem?
                if let Some(res) = context.get_static_content(path_without_slash_start) {
                    println!(">>> static content: {}", path);
                    let ext = path.rsplit('.').next();
                    let content_type = match ext {
                        None => "text/plain; charset=utf-8",
                        Some("js") => "application/javascript; charset=utf-8",
                        Some("css") => "text/css; charset=utf-8",
                        Some("map") | Some("json") => "application/json; charset=utf-8",
                        Some(_) => "text/plain; charset=utf-8",
                    };
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::OK)
                        .header(CONTENT_TYPE, content_type)
                        .body(hyper::Body::from(res))
                        .unwrap());
                }
                // for hmr files
                let res = staticfile.serve(req).await;
                res.map_err(anyhow::Error::from)
            }
        }
    }

    // TODO: refact socket message data structure
    async fn handle_websocket(
        websocket: hyper_tungstenite::HyperWebsocket,
        mut receiver: broadcast::Receiver<WsMessage>,
    ) -> Result<()> {
        let websocket = websocket.await?;
        let (mut sender, mut ws_recv) = websocket.split();
        let task = tokio::spawn(async move {
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
        // sender.send(Message::text(r#"{"hash":"initial"}"#)).await?;
        while let Some(message) = ws_recv.next().await {
            if let Ok(Message::Close(_)) = message {
                break;
            }
        }
        task.abort();
        Ok(())
    }

    fn watch_for_changes(
        root: PathBuf,
        compiler: Arc<Compiler>,
        txws: broadcast::Sender<WsMessage>,
        callback: impl Fn(OnDevCompleteParams) + Clone,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        // let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
        let mut debouncer = new_debouncer(Duration::from_millis(0), None, tx).unwrap();
        let watcher = debouncer.watcher();
        Watch::watch(&root, watcher)?;

        let initial_hash = compiler.full_hash();
        let mut last_cache_hash = Box::new(initial_hash);
        let mut hmr_hash = Box::new(initial_hash);

        for result in rx {
            let events = Watch::normalize_events(result.unwrap());
            if !events.is_empty() {
                let compiler = compiler.clone();
                let txws = txws.clone();
                let callback = callback.clone();
                if let Err(e) = Self::rebuild(
                    events,
                    compiler,
                    txws,
                    &mut last_cache_hash,
                    &mut hmr_hash,
                    callback,
                ) {
                    eprintln!("Error rebuilding: {:?}", e);
                }
            }
        }
        Ok(())
    }

    fn rebuild(
        events: Vec<WatchEvent>,
        compiler: Arc<Compiler>,
        txws: broadcast::Sender<WsMessage>,
        last_cache_hash: &mut Box<u64>,
        hmr_hash: &mut Box<u64>,
        callback: impl Fn(OnDevCompleteParams),
    ) -> Result<()> {
        debug!("watch events detected: {:?}", events);
        debug!("checking update status...");
        let res = compiler.update(events);
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

        if let Err(e) = res {
            debug!("checking update status... failed");
            println!("Compiling...");
            eprintln!("{}: {}", "error".red(), e);
            return Err(e);
        }

        let res = res.unwrap();
        let is_updated = res.is_updated();
        debug!("update status is ok, is_updated: {}", is_updated);
        if !is_updated {
            return Ok(());
        }

        println!("Compiling...");
        let t_compiler = Instant::now();
        let start_time = std::time::SystemTime::now();
        let next_hash = compiler.generate_hot_update_chunks(res, **last_cache_hash, **hmr_hash);
        debug!(
            "hot update chunks generated, next_full_hash: {:?}",
            next_hash
        );
        // do not print hot rebuilt message if there are missing deps
        // since it's not a success rebuilt to user
        if !has_missing_deps {
            println!(
                "Hot rebuilt in {}",
                format!("{}ms", t_compiler.elapsed().as_millis()).bold()
            );
        }
        if let Err(e) = next_hash {
            eprintln!("Error in watch: {:?}", e);
            return Err(e);
        }
        let (next_cache_hash, next_hmr_hash) = next_hash.unwrap();
        debug!(
            "hash info, next: {:?}, last: {:?}, is_equal: {}",
            next_cache_hash,
            last_cache_hash,
            next_cache_hash == **last_cache_hash
        );
        if next_cache_hash == **last_cache_hash {
            debug!("hash equals, will not do full rebuild");
            return Ok(());
        } else {
            **last_cache_hash = next_cache_hash;
            **hmr_hash = next_hmr_hash;
        }

        debug!("full rebuild...");
        if let Err(e) = compiler.emit_dev_chunks(next_cache_hash, next_hmr_hash) {
            debug!("  > build failed: {:?}", e);
            return Err(e);
        }
        debug!("full rebuild...done");
        if !has_missing_deps {
            println!(
                "Full rebuilt in {}",
                format!("{}ms", t_compiler.elapsed().as_millis()).bold()
            );

            let end_time = std::time::SystemTime::now();
            callback(OnDevCompleteParams {
                is_first_compile: false,
                time: t_compiler.elapsed().as_millis() as u64,
                stats: Stats {
                    start_time: start_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                    end_time: end_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                },
            });
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

pub struct OnDevCompleteParams {
    pub is_first_compile: bool,
    pub time: u64,
    pub stats: Stats,
}

pub struct Stats {
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Clone, Debug)]
struct WsMessage {
    hash: u64,
}
