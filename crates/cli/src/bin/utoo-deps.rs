use anyhow::Result;
use clap::Parser;
use utoo_cli::{
    cmd::deps::{build_deps, build_workspace},
    constants::{cmd::DEPS_ABOUT, APP_VERSION},
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

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let result = if cli.workspace_only {
        build_workspace().await
    } else {
        build_deps().await
    };

    result?;
    Ok(())
}
