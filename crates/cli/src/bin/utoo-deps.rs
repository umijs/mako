use clap::Parser;
use std::process;
use utoo_cli::{
    cmd::deps::{build_deps, build_workspace},
    constants::{cmd::DEPS_ABOUT, APP_VERSION},
    util::logger::{log_error, write_verbose_logs_to_file},
};

#[derive(Parser)]
#[command(
    name = "utoo-deps",
    version = APP_VERSION,
    about = DEPS_ABOUT
)]
struct Cli {
    /// Only build workspace dependencies
    #[arg(long = "workspace-only")]
    workspace_only: bool,

    /// Show detailed dependency information
    #[arg(short, long)]
    detail: bool,

    /// Show only production dependencies
    #[arg(long)]
    prod: bool,

    /// Show only development dependencies
    #[arg(long)]
    dev: bool,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = if cli.workspace_only {
        build_workspace().await
    } else {
        build_deps().await
    };

    if let Err(e) = result {
        log_error(&e.to_string());
        let _ = write_verbose_logs_to_file();
        process::exit(1);
    }
}
