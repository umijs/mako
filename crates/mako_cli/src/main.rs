use std::path::PathBuf;

use clap::Parser;
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use mako_bundler::{config, Bundler};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct MakoCLI {
    #[arg(short, long, default_value_t = false)]
    watch: bool,
    root: PathBuf,
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("mako_bundler=debug")),
        )
        .without_time()
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    let cli: MakoCLI = MakoCLI::parse();

    let config = config::Config {
        root: std::env::current_dir().unwrap().join(cli.root.clone()),
        externals: maplit::hashmap! {
            "stream".to_string() => "stream".to_string()
        },
        entry: maplit::hashmap! {
            "index".to_string() => "index.tsx".to_string().into()
        },
        ..Default::default()
    };

    let mut b = Bundler::new(config);
    b.run(cli.watch).unwrap();
}
