use std::path::PathBuf;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use hyper::Server;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::Instant;
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
        let watch_handler = self.watcher.start();

        async fn serve_websocket(
            websocket: hyper_tungstenite::HyperWebsocket,
            mut rx: Receiver<()>,
        ) -> Result<(), Error> {
            let websocket = websocket.await?;

            let (mut sender, mut ws_recv) = websocket.split();

            // sender.send(Message::text("{}")).await?;

            let fwd_task = tokio::spawn(async move {
                loop {
                    if (rx.recv().await).is_ok()
                        && sender
                            .send(Message::text(r#"{"update": "todo"}"#))
                            .await
                            .is_err()
                    {
                        break;
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
                            Ok(res) => Ok(res),
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

        let _ = tokio::join!(watch_handler, dev_server_handle);
    }
}

struct ProjectWatch {
    root: PathBuf,
    compiler: std::sync::Arc<compiler::Compiler>,
    tx: Sender<()>,
}

impl ProjectWatch {
    pub fn new(root: PathBuf, c: Arc<compiler::Compiler>) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel::<()>(256);
        Self {
            compiler: c,
            root,
            tx,
        }
    }

    pub fn start(&self) -> JoinHandle<()> {
        let c = self.compiler.clone();
        let root = self.root.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            watch(&root, |events| {
                let res = c.update(events.into()).unwrap();

                if res.is_updated() {
                    c.generate_hot_update_chunks(res);

                    if tx.receiver_count() > 0 {
                        tx.send(()).unwrap();
                    }

                    let c = c.clone();
                    tokio::spawn(async move {
                        let _t = Instant::now();

                        c.generate().unwrap();
                    });
                }
            });
        })
    }

    pub fn clone_receiver(&self) -> Receiver<()> {
        self.tx.subscribe()
    }
}
