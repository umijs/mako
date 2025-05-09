use anyhow::Result;
use clap::Parser;
use utoo_cli::{
    cmd::update::update,
    constants::{cmd::UPDATE_ABOUT, APP_VERSION},
    util::logger::{log_error, write_verbose_logs_to_file},
};

#[derive(Parser)]
#[command(
    name = "utoo-update",
    version = APP_VERSION,
    about = UPDATE_ABOUT
)]
struct Cli {
    #[arg(long = "ignore-scripts")]
    ignore_scripts: bool,

    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    update(cli.ignore_scripts).await?;
    Ok(())
}
