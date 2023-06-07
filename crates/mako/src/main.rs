#![feature(box_patterns)]

use std::sync::Arc;

use clap::Parser;
use hyper::service::service_fn;
use hyper::Server;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

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
        let watch_handler = tokio::spawn(async move {
            watch(&root, |events| {
                info!("chang event {:?}", events);

                let c = watch_compiler.clone();
                let res = c.update(events.into()).unwrap();
                dbg!(&res);
                c.generate_with_update(res);
            });
        });

        let handle_nf = move |req: hyper::Request<hyper::Body>| {
            let for_fn = arc_compiler.clone();
            async move {
                let path = req.uri().path().strip_prefix('/').or({ Some("") }).unwrap();

                dbg!(&path);

                match path {
                    "" | "index.html" | "index.htm" => {
                        let index = std::fs::read(
                            for_fn.context.config.output.path.clone().join("index.html"),
                        )
                        .unwrap();

                        Ok::<_, hyper::Error>(
                            hyper::Response::builder()
                                .header("content-type", "text/html")
                                .body(hyper::Body::from(index))
                                .unwrap(),
                        )
                    }
                    _ => {
                        if let Some(chunk) = for_fn.get_chunk_content_by_path(path.to_string()) {
                            Ok::<_, hyper::Error>(hyper::Response::new(hyper::Body::from(chunk)))
                        } else {
                            Ok::<_, hyper::Error>(
                                hyper::Response::builder()
                                    .status(hyper::StatusCode::NOT_FOUND)
                                    .body(hyper::Body::from("404 - Page not found"))
                                    .unwrap(),
                            )
                        }
                    }
                }
            }
        };
        let dev_service = hyper::service::make_service_fn(move |_conn| {
            let my_fn = handle_nf.clone();
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
