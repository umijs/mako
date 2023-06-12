use crate::compiler;
use crate::compiler::Compiler;
use crate::watch::watch;
use futures::{SinkExt, StreamExt};
use hyper::Server;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::task::JoinHandle;
use tungstenite::Message;

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

            sender.send(Message::text("{}")).await?;

            let fwd_task = tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(_) => {
                            if sender
                                .send(Message::text(r#"{"update": "todo"}"#))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        _ => {}
                    };
                }
            });

            while let Some(message) = ws_recv.next().await {
                match message.unwrap() {
                    Message::Close(_) => {
                        break;
                    }
                    _ => {}
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
                        if let Some(chunk) = for_fn.get_chunk_content_by_path(path.to_string()) {
                            Ok::<_, hyper::Error>(hyper::Response::new(hyper::Body::from(chunk)))
                        } else {
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
            }
        };
        let dev_service = hyper::service::make_service_fn(move |_conn| {
            let my_fn = handle_request.clone();
            async move { Ok::<_, hyper::Error>(hyper::service::service_fn(my_fn)) }
        });

        let dev_server_handle = tokio::spawn(async move {
            if let Err(_e) = Server::bind(&([127, 0, 0, 1], 3000).into())
                .serve(dev_service)
                .await
            {
                println!("done");
            }
        });

        tokio::join!(watch_handler, dev_server_handle);
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
                c.generate_with_update(res);
                if tx.receiver_count() > 0 {
                    tx.send(()).unwrap();
                }
            });
        })
    }

    pub fn clone_receiver(&self) -> Receiver<()> {
        self.tx.subscribe()
    }
}
