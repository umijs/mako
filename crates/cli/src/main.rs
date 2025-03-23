use std::process;

use clap::{Parser, Subcommand};
use cmd::deps::build_deps;
use cmd::install::install;
use cmd::rebuild::rebuild;
use cmd::{clean::clean, deps::build_workspace};
use helper::auto_update::init_auto_update;
use util::config::set_registry;
use util::logger::{log_error, log_info, log_warning, set_verbose, write_verbose_logs_to_file};

mod cmd;
mod constants;
mod helper;
mod model;
mod service;
mod util;

use crate::constants::cmd::{
    CLEAN_ABOUT, CLEAN_NAME, DEPS_ABOUT, DEPS_NAME, INSTALL_ABOUT, INSTALL_NAME, REBUILD_ABOUT,
    REBUILD_NAME,
};
use crate::constants::{APP_ABOUT, APP_NAME, APP_VERSION};

#[derive(Parser)]
#[command(name = APP_NAME)]
#[command(version = APP_VERSION)]
#[command(about = APP_ABOUT)]
struct Cli {
    #[arg(long)]
    ignore_scripts: bool,

    #[arg(short = 'V', long, global = true)]
    verbose: bool,

    #[arg(short, long)]
    version: bool,

    #[arg(long, global = true, default_value = "https://registry.npmmirror.com")]
    registry: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = INSTALL_NAME, alias = "i", about = INSTALL_ABOUT)]
    Install {
        #[arg(long)]
        ignore_scripts: bool,
    },

    #[command(name = REBUILD_NAME, alias = "rb", about = REBUILD_ABOUT)]
    Rebuild,

    #[command(name = CLEAN_NAME, alias = "c", about = CLEAN_ABOUT)]
    Clean {
        #[arg(default_value = "*")]
        pattern: String,
    },

    #[command(name = DEPS_NAME, alias = "d", about = DEPS_ABOUT)]
    Deps {
        #[arg(long)]
        workspace_only: bool,
    },
}

#[tokio::main]
async fn main() {
    unsafe {
        libc::umask(0o00);
    }

    // Ensure the version is up to date, weak dependency
    if let Err(_e) = init_auto_update().await {
        log_warning(&format!("Auto update cancelled"));
    }

    let cli = Cli::parse();

    // global verbose
    set_verbose(cli.verbose);

    // global registry
    set_registry(&cli.registry);

    // load package.json
    // if let Some(result) = cmd::pkg::handle_command_v4(&cli) {
    //     if let Err(e) = result {
    //         log_error(&e);
    //     }
    //     return;
    // }

    match cli.command {
        Some(Commands::Clean { pattern }) => {
            if let Err(e) = clean(&pattern).await {
                log_error(&e.to_string());
                let _ = write_verbose_logs_to_file();
                process::exit(1);
            }
        }
        Some(Commands::Install { ignore_scripts }) => {
            if let Err(e) = install(ignore_scripts).await {
                log_error(&e.to_string());
                let _ = write_verbose_logs_to_file();
                process::exit(1);
            }
        }
        Some(Commands::Rebuild) => {
            log_info("Executing dependency hook scripts and creating node_modules/.bin links");
            if let Err(e) = rebuild().await {
                log_error(&e);
                process::exit(1);
            }
            log_info("💫 All dependencies rebuild completed");
        }
        Some(Commands::Deps { workspace_only }) => {
            let result = if workspace_only {
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
        None => {
            // install by default
            if let Err(e) = install(cli.ignore_scripts).await {
                log_error(&e.to_string());
                let _ = write_verbose_logs_to_file();
                process::exit(1);
            }
        }
    }
}
