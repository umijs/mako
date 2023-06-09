#![feature(box_patterns)]

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use futures::stream::StreamExt;
use futures::SinkExt;
use hyper::service::service_fn;
use hyper::Server;

use tokio::task::JoinHandle;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use tungstenite::protocol::Message;

use crate::watch::watch;

mod analyze_deps;
mod ast;
mod bfs;
mod build;
mod chunk;
mod chunk_graph;
mod cli;
mod compiler;
mod config;
mod config_node_polyfill;
mod copy;
mod generate;
mod generate_chunks;
mod group_chunk;
mod hmr;
mod load;
mod minify;
mod module;
mod module_graph;
mod parse;
mod resolve;
mod sourcemap;
#[cfg(test)]
mod test_helper;
mod transform;
mod transform_css_handler;
mod transform_dep_replacer;
mod transform_dynamic_import;
mod transform_env_replacer;
mod transform_in_generate;
mod transform_optimizer;
mod update;
mod watch;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() {
    // logger
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mako=info")),
        )
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .without_time()
        .init();

    // cli
    let cli = cli::Cli::parse();
    debug!(
        "cli: watch = {}, mode = {}, root = {}",
        cli.watch,
        cli.mode,
        cli.root.to_str().unwrap()
    );
    let root = if cli.root.is_absolute() {
        cli.root
    } else {
        std::env::current_dir().unwrap().join(cli.root)
    };

    // config
    let mut config = config::Config::new(&root).unwrap();
    config.mode = cli.mode;
    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone());
    compiler.compile();

    let arc_compiler = Arc::new(compiler);

    let watch_compiler = arc_compiler.clone();
    if cli.watch {
        let w = ProjectWatch {
            root: root.clone(),
            compiler: watch_compiler,
        };

        let watch_handler = w.start();

        async fn serve_websocket(
            websocket: hyper_tungstenite::HyperWebsocket,
        ) -> Result<(), Error> {
            let mut websocket = websocket.await?;

            websocket.send(Message::text("hello from mako")).await?;

            while let Some(message) = websocket.next().await {
                match message? {
                    Message::Close(msg) => {
                        // No need to send a reply: tungstenite takes care of this for you.
                        if let Some(msg) = &msg {
                            println!(
                                "Received close message with code {} and message: {}",
                                msg.code, msg.reason
                            );
                        } else {
                            println!("Received close message");
                        }
                        break;
                    }
                    _ => {}
                }
            }

            Ok(())
        }

        let handle_request = move |req: hyper::Request<hyper::Body>| {
            let for_fn = arc_compiler.clone();
            async move {
                let path = req.uri().path().strip_prefix('/').unwrap_or("");

                let static_serve =
                    hyper_staticfile::Static::new(for_fn.context.config.output.path.clone());
                dbg!(&path);

                match path {
                    "__/hmr-ws" => {
                        if hyper_tungstenite::is_upgrade_request(&req) {
                            let (response, websocket) =
                                hyper_tungstenite::upgrade(req, None).unwrap();

                            tokio::spawn(async move {
                                if let Err(e) = serve_websocket(websocket).await {
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
            async move { Ok::<_, hyper::Error>(service_fn(my_fn)) }
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
    compiler: Arc<compiler::Compiler>,
}

impl ProjectWatch {
    pub fn start(&self) -> JoinHandle<()> {
        let c = self.compiler.clone();
        let root = self.root.clone();
        tokio::spawn(async move {
            watch(&root, |events| {
                info!("chang event {:?}", events);

                let res = c.update(events.into()).unwrap();
                dbg!(&res);
                c.generate_with_update(res);
            });
        })
    }

    pub fn add_listener(&self) {}
}
