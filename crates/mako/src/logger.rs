use mako_core::tracing_subscriber::{fmt, EnvFilter};

pub fn init_logger() {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mako=info")),
        )
        .with_span_events(fmt::format::FmtSpan::NONE)
        .without_time()
        .init();
}
