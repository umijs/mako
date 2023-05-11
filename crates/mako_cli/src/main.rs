use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

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

    mako_bundler::run();
}
