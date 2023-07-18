use std::path::PathBuf;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use hyper::header::CONTENT_TYPE;
use hyper::http::HeaderValue;
use hyper::Server;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;
use tokio::try_join;
use tracing::debug;
use tungstenite::Message;

use crate::compiler;
use crate::compiler::Compiler;
use crate::watch::watch;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct DevServer {
    watcher: Arc<ProjectWatch>,
    compiler: Arc<Compiler>,
}

impl DevServer {
    pub fn new(root: PathBuf, compiler: Arc<Compiler>) -> Self {
        Self {
            watcher: Arc::new(ProjectWatch::new(root, compiler.clone())),
            compiler,
        }
    }

    pub async fn serve(&self) {
        let (watch_handler, build_handle) = self.watcher.start();

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
                if let Message::Close(_) = message.unwrap() {
                    break;
                }
            }

            // release rx;
            fwd_task.abort();

            Ok(())
        }
        let arc_watcher = Arc::new(self.watcher.clone());
        let compiler = self.compiler.clone();
        let handle_request = move |req: hyper::Request<hyper::Body>| {
            let for_fn = compiler.clone();
            let r = arc_watcher.clone_receiver();
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
                                if let Err(e) = serve_websocket(websocket, r).await {
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
                    _ => {
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
            if let Err(_e) = Server::bind(&([127, 0, 0, 1], port).into())
                .serve(dev_service)
                .await
            {
                println!("done");
            }
        });

        let _ = try_join!(watch_handler, dev_server_handle, build_handle);
    }
}

#[derive(Clone, Debug)]
struct WsMessage {
    hash: u64,
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

    pub fn start(&self) -> (JoinHandle<()>, JoinHandle<()>) {
        let c = self.compiler.clone();
        let root = self.root.clone();
        let tx = self.tx.clone();

        let mut last_full_hash = Box::new(c.full_hash());

        let (build_tx, mut build_rx) = unbounded_channel::<()>();

        let watch_compiler = c.clone();
        let watch_handle = tokio::spawn(async move {
            watch(&root, |events| {
                let res = watch_compiler.update(events.into());

                match res {
                    Err(e) => {
                        eprintln!("Error in watch: {:?}", e);
                    }
                    Ok(res) => {
                        if res.is_updated() {
                            let next_full_hash =
                                watch_compiler.generate_hot_update_chunks(res, *last_full_hash);

                            if let Err(e) = next_full_hash {
                                eprintln!("Error in watch: {:?}", e);
                                return;
                            }

                            let next_full_hash = next_full_hash.unwrap();

                            if next_full_hash == *last_full_hash {
                                // no need to continue
                                return;
                            } else {
                                *last_full_hash = next_full_hash;
                            }

                            if tx.receiver_count() > 0 {
                                tx.send(WsMessage {
                                    hash: next_full_hash,
                                })
                                .unwrap();
                            }

                            let _ = build_tx.send(());
                        }
                    }
                }
            });
        });

        let build_handle = tokio::spawn(async move {
            while (build_rx.recv().await).is_some() {
                // Then try to receive all remaining messages immediately.
                while build_rx.try_recv().is_ok() {}

                if let Err(e) = c
                    .generate_chunks_ast()
                    .and_then(|chunk_asts| c.emit_dev_chunks(chunk_asts))
                {
                    debug!("Error in build: {:?}, will rebuild soon", e);
                }
            }
        });

        (watch_handle, build_handle)
    }

    pub fn clone_receiver(&self) -> Receiver<WsMessage> {
        self.tx.subscribe()
    }
}
