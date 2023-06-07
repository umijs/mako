use tracing_subscriber::EnvFilter;

pub fn init_logger() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mako=info")),
        )
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .without_time()
        .init();
}
