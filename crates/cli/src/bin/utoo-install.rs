use clap::Parser;
use std::process;
use utoo_cli::{
    cmd::install::install,
    constants::{cmd::INSTALL_ABOUT, APP_VERSION},
    util::logger::{log_error, write_verbose_logs_to_file},
};

#[derive(Parser)]
#[command(
    name = "utoo-install",
    version = APP_VERSION,
    about = INSTALL_ABOUT
)]
struct Cli {
    /// Skip running scripts during installation
    #[arg(long = "ignore-scripts")]
    ignore_scripts: bool,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = install(cli.ignore_scripts).await {
        log_error(&e.to_string());
        let _ = write_verbose_logs_to_file();
        process::exit(1);
    }
}
