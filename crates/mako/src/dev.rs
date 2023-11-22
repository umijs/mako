use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mako_core::colored::Colorize;
use mako_core::futures::{SinkExt, StreamExt};
use mako_core::hyper::header::CONTENT_TYPE;
use mako_core::hyper::http::HeaderValue;
use mako_core::hyper::server::conn::AddrIncoming;
use mako_core::hyper::server::Builder;
use mako_core::hyper::{Body, Request, Server};
use mako_core::lazy_static::lazy_static;
use mako_core::rayon::ThreadPoolBuilder;
use mako_core::regex::Regex;
use mako_core::tokio::sync::broadcast::{Receiver, Sender};
use mako_core::tokio::try_join;
use mako_core::tracing::debug;
use mako_core::tungstenite::Message;
use mako_core::{hyper, hyper_staticfile, hyper_tungstenite, tokio};

use crate::compiler;
use crate::compiler::Compiler;
use crate::watch::watch;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

fn bind_idle_port(port: u16) -> Builder<AddrIncoming> {
    let mut port = port;
    // 循环调用 try_bind, err 继续寻找端口, ok 返回实例
    loop {
        match Server::try_bind(&([127, 0, 0, 1], port).into()) {
            Ok(builder) => {
                return builder;
            }
            Err(_) => {
                port += 1;
            }
        }
    }
}

pub struct DevServer {
    watcher: Arc<ProjectWatch>,
    compiler: Arc<Compiler>,
}

lazy_static! {
    static ref HOTUPDATE_RES_REGEX: Regex =
        Regex::new(r#"\.hot-update\.(css|js|json|js\.map|css\.map)$"#).unwrap();
}

impl DevServer {
    pub fn new(root: PathBuf, compiler: Arc<Compiler>) -> Self {
        Self {
            watcher: Arc::new(ProjectWatch::new(root, compiler.clone())),
            compiler,
        }
    }

    pub async fn serve(&self, callback: impl Fn(OnDevCompleteParams) + Send + Sync + 'static) {
        self.watcher.start(callback);

        async fn serve_websocket(
            websocket: hyper_tungstenite::HyperWebsocket,
            mut rx: Receiver<WsMessage>,
        ) -> Result<(), Error> {
            let websocket = websocket.await?;

            let (mut sender, mut ws_recv) = websocket.split();

            let fwd_task = tokio::spawn(async move {
                loop {
                    if let Ok(msg) = rx.recv().await {
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

            // release rx;
            fwd_task.abort();

            Ok(())
        }
        let arc_watcher = self.watcher.clone();
        let compiler = self.compiler.clone();
        let handle_request = move |req: Request<Body>| {
            let for_fn = compiler.clone();
            let w = arc_watcher.clone();
            async move {
                let path = req.uri().path().strip_prefix('/').unwrap_or("");

                let static_serve =
                    hyper_staticfile::Static::new(for_fn.context.config.output.path.clone());

                match path {
                    "__/hmr-ws" => {
                        if hyper_tungstenite::is_upgrade_request(&req) {
                            let (response, websocket) =
                                hyper_tungstenite::upgrade(req, None).unwrap();

                            tokio::spawn(async move {
                                if let Err(e) = serve_websocket(websocket, w.clone_receiver()).await
                                {
                                    eprintln!("Error in websocket connection: {}", e);
                                }
                            });

                            Ok(response)
                        } else {
                            Ok::<_, hyper::Error>(
                                hyper::Response::builder()
                                    .status(hyper::StatusCode::NOT_FOUND)
                                    .body(hyper::Body::empty())
                                    .unwrap(),
                            )
                        }
                    }

                    path if HOTUPDATE_RES_REGEX.is_match(path) => {
                        match static_serve.serve(req).await {
                            Ok(mut res) => {
                                if let Some(content_type) = res.headers().get(CONTENT_TYPE).cloned()
                                {
                                    if let Ok(c_str) = content_type.to_str() {
                                        if c_str.contains("javascript") || c_str.contains("text") {
                                            res.headers_mut()
                                                .insert(
                                                    CONTENT_TYPE,
                                                    HeaderValue::from_str(&format!(
                                                        "{c_str}; charset=utf-8"
                                                    ))
                                                    .unwrap(),
                                                )
                                                .unwrap();
                                        }
                                    }
                                }
                                Ok(res)
                            }
                            Err(_) => Ok::<_, hyper::Error>(
                                hyper::Response::builder()
                                    .status(hyper::StatusCode::NOT_FOUND)
                                    .body(hyper::Body::from("404 - Page not found"))
                                    .unwrap(),
                            ),
                        }
                    }
                    _ => {
                        if let Some(res) = for_fn.context.get_static_content(path) {
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

                        // try chunk content in memory first, else use dist content
                        match static_serve.serve(req).await {
                            Ok(mut res) => {
                                if let Some(content_type) = res.headers().get(CONTENT_TYPE).cloned()
                                {
                                    if let Ok(c_str) = content_type.to_str() {
                                        if c_str.contains("javascript") || c_str.contains("text") {
                                            res.headers_mut()
                                                .insert(
                                                    CONTENT_TYPE,
                                                    HeaderValue::from_str(&format!(
                                                        "{c_str}; charset=utf-8"
                                                    ))
                                                    .unwrap(),
                                                )
                                                .unwrap();
                                        }
                                    }
                                }
                                Ok(res)
                            }
                            Err(_) => Ok::<_, hyper::Error>(
                                hyper::Response::builder()
                                    .status(hyper::StatusCode::NOT_FOUND)
                                    .body(hyper::Body::from("404 - Page not found"))
                                    .unwrap(),
                            ),
                        }
                    }
                }
            }
        };
        let dev_service = hyper::service::make_service_fn(move |_conn| {
            let my_fn = handle_request.clone();
            async move { Ok::<_, hyper::Error>(hyper::service::service_fn(my_fn)) }
        });

        let port = self.compiler.context.config.hmr_port.clone();
        let port = port.parse::<u16>().unwrap();

        let dev_server_handle = tokio::spawn(async move {
            if let Err(_e) = bind_idle_port(port).serve(dev_service).await {
                println!("done");
            }
        });

        // build_handle 必须在 dev_server_handle 之前
        // 否则会导致 build_handle 无法收到前几个消息，原因未知
        let join_error = try_join!(dev_server_handle);
        if let Err(e) = join_error {
            eprintln!("Error in dev server: {:?}", e);
        }
    }
}

#[derive(Clone, Debug)]
struct WsMessage {
    hash: u64,
}

struct RebuildMessage {
    t_compiler: Instant,
    start_time: SystemTime,
    next_cache_hash: u64,
    next_hmr_hash: u64,
    has_missing_deps: bool,
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

struct ProjectWatch {
    root: PathBuf,
    compiler: std::sync::Arc<compiler::Compiler>,
    tx: Sender<WsMessage>,
}

impl ProjectWatch {
    pub fn new(root: PathBuf, c: Arc<compiler::Compiler>) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel::<WsMessage>(256);
        Self {
            compiler: c,
            root,
            tx,
        }
    }

    pub fn start(&self, callback: impl Fn(OnDevCompleteParams) + Send + Sync + 'static) {
        let c = self.compiler.clone();
        let root = self.root.clone();
        let tx = self.tx.clone();
        // full rebuild channel
        let (build_send, build_resv) = mpsc::channel::<RebuildMessage>();

        let initial_hash = c.full_hash();

        let mut last_cache_hash = Box::new(initial_hash);
        let mut hmr_hash = Box::new(initial_hash);
        debug!("last_full_hash: {:?}", last_cache_hash);

        let watch_compiler = c.clone();

        let pool = ThreadPoolBuilder::new().build().unwrap();
        pool.spawn(move || {
            watch(&root, |events| {
                debug!("watch events detected: {:?}", events);
                debug!("checking update status...");
                let res = watch_compiler.update(events.into());
                let has_missing_deps = {
                    watch_compiler
                        .context
                        .modules_with_missing_deps
                        .read()
                        .unwrap()
                        .len()
                        > 0
                };
                debug!("has_missing_deps: {}", has_missing_deps);
                debug!("checking update status... done");

                match res {
                    Err(err) => {
                        debug!("update status is error: {:?}", err);
                        println!("Compiling...");
                        // unescape
                        let mut err = err
                            .to_string()
                            .replace("\\n", "\n")
                            .replace("\\u{1b}", "\u{1b}")
                            .replace("\\\\", "\\");
                        // remove first char and last char
                        if err.starts_with('"') && err.ends_with('"') {
                            err = err[1..err.len() - 1].to_string();
                        }
                        eprintln!("{}", "Build failed.".to_string().red());
                        eprintln!("{}", err);
                    }
                    Ok(res) => {
                        debug!("update status is ok, is_updated: {}", res.is_updated());
                        if res.is_updated() {
                            println!("Compiling...");
                            let t_compiler = Instant::now();
                            let start_time = std::time::SystemTime::now();
                            let next_hash = watch_compiler.generate_hot_update_chunks(
                                res,
                                *last_cache_hash,
                                *hmr_hash,
                            );
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
                                return;
                            }

                            let (next_cache_hash, next_hmr_hash) = next_hash.unwrap();
                            debug!(
                                "hash info, next: {:?}, last: {:?}, is_equal: {}",
                                next_cache_hash,
                                last_cache_hash,
                                next_cache_hash == *last_cache_hash
                            );
                            if next_cache_hash == *last_cache_hash {
                                debug!("hash equals, will not do full rebuild");
                                return;
                            } else {
                                *last_cache_hash = next_cache_hash;
                                *hmr_hash = next_hmr_hash;
                                build_send
                                    .send(RebuildMessage {
                                        t_compiler,
                                        start_time,
                                        next_cache_hash,
                                        next_hmr_hash,
                                        has_missing_deps,
                                    })
                                    .unwrap();
                            }

                            debug!("receiver count: {}", tx.receiver_count());
                            if tx.receiver_count() > 0 {
                                tx.send(WsMessage { hash: *hmr_hash }).unwrap();
                                debug!("send message to clients");
                            }
                        }
                    }
                }
            });
        });

        pool.spawn(move || {
            loop {
                match build_resv.recv() {
                    Ok(rebuild_msg) => {
                        let mut chunk_cache = c.context.static_cache.write().unwrap();

                        let mut last_msg = rebuild_msg;

                        // 查看通道里还有没有未处理的消息，有的话统一处理，减少 rebuild 次数
                        while let Ok(msg) = build_resv.try_recv() {
                            last_msg = msg;
                        }

                        debug!("full rebuild...");
                        if let Err(e) = c.emit_dev_chunks(
                            last_msg.next_cache_hash,
                            last_msg.next_hmr_hash,
                            chunk_cache.deref_mut(),
                        ) {
                            debug!("  > build failed: {:?}", e);
                            return;
                        }
                        debug!("full rebuild...done");
                        if !last_msg.has_missing_deps {
                            println!(
                                "Full rebuilt in {}",
                                format!("{}ms", last_msg.t_compiler.elapsed().as_millis()).bold()
                            );

                            let end_time = std::time::SystemTime::now();
                            callback(OnDevCompleteParams {
                                is_first_compile: false,
                                time: last_msg.t_compiler.elapsed().as_millis() as u64,
                                stats: Stats {
                                    start_time: last_msg
                                        .start_time
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis()
                                        as u64,
                                    end_time: end_time
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis()
                                        as u64,
                                },
                            });
                        }
                    }
                    Err(_) => {
                        println!("Channel closed");
                        break;
                    }
                }
            }
        })
    }

    pub fn clone_receiver(&self) -> Receiver<WsMessage> {
        self.tx.subscribe()
    }
}
