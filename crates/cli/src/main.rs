use std::process;

use clap::{Parser, Subcommand};
use cmd::deps::build_deps;
use cmd::install::{install, update_package};
use cmd::rebuild::rebuild;
use cmd::update::update;
use cmd::{clean::clean, deps::build_workspace};
use helper::auto_update::init_auto_update;
use util::config::{set_legacy_peer_deps, set_registry};
use util::logger::{log_error, log_info, log_warning, set_verbose, write_verbose_logs_to_file};
use util::save_type::{parse_save_type, PackageAction, SaveType};

mod cmd;
mod constants;
mod helper;
mod model;
mod service;
mod util;

use crate::constants::cmd::{
    CLEAN_ABOUT, CLEAN_NAME, DEPS_ABOUT, DEPS_NAME, INSTALL_ABOUT, INSTALL_NAME, REBUILD_ABOUT,
    REBUILD_NAME, UNINSTALL_ABOUT, UNINSTALL_NAME, UPDATE_ABOUT,
};
use crate::constants::{APP_ABOUT, APP_NAME, APP_VERSION};

#[derive(Parser)]
#[command(name = APP_NAME)]
#[command(version = APP_VERSION)]
#[command(about = APP_ABOUT)]
struct Cli {
    #[arg(long)]
    ignore_scripts: bool,

    #[arg(long, global = true)]
    verbose: bool,

    #[arg(short, long)]
    version: bool,

    #[arg(long, global = true)]
    registry: Option<String>,

    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    legacy_peer_deps: Option<bool>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install dependencies
    #[command(name = INSTALL_NAME, alias = "i", about = INSTALL_ABOUT)]
    Install {
        /// Package specification (e.g. "lodash@4.17.21")
        spec: Option<String>,

        /// Workspace to install in
        #[arg(short, long)]
        workspace: Option<String>,

        /// Skip running dependency scripts
        #[arg(long)]
        ignore_scripts: bool,

        /// Save as dev dependency
        #[arg(long)]
        save_dev: bool,

        /// Save as peer dependency
        #[arg(long)]
        save_peer: bool,

        /// Save as optional dependency
        #[arg(long)]
        save_optional: bool,
    },
    /// Uninstall dependencies
    #[command(name = UNINSTALL_NAME, alias = "un", about = UNINSTALL_ABOUT)]
    Uninstall {
        /// Package specification (e.g. "lodash@4.17.21")
        spec: Option<String>,

        /// Workspace to uninstall from
        #[arg(short, long)]
        workspace: Option<String>,

        /// Skip running dependency scripts
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

    #[command(name = "update", alias = "u", about = UPDATE_ABOUT)]
    Update,

    /// Run scripts defined in package.json
    #[command(name = "run", alias = "r")]
    Run {
        /// Script name to run
        script: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        libc::umask(0o00);
    }

    let cli = Cli::parse();

    // global verbose
    set_verbose(cli.verbose);

    // global registry
    set_registry(cli.registry);

    // set legacy_peer_deps when set --legacy
    if cli.legacy_peer_deps == Some(true) {
        set_legacy_peer_deps(cli.legacy_peer_deps);
    }

    // Ensure the version is up to date, weak dependency
    if let Err(_e) = init_auto_update().await {
        log_warning(&format!("Auto update cancelled"));
    }

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
        Some(Commands::Install {
            spec,
            workspace,
            ignore_scripts,
            save_dev,
            save_peer,
            save_optional,
        }) => {
            if let Some(spec) = spec {
                let save_type = parse_save_type(save_dev, save_peer, save_optional);
                if let Err(e) = update_package(
                    PackageAction::Add,
                    &spec,
                    workspace.clone(),
                    ignore_scripts,
                    save_type,
                )
                .await
                {
                    log_error(&e.to_string());
                    let _ = write_verbose_logs_to_file();
                    process::exit(1);
                }
            } else {
                if let Err(e) = install(ignore_scripts).await {
                    log_error(&e.to_string());
                    let _ = write_verbose_logs_to_file();
                    process::exit(1);
                }
            }
        }
        Some(Commands::Uninstall {
            spec,
            workspace,
            ignore_scripts,
        }) => {
            if let Some(spec) = spec {
                if let Err(e) = update_package(
                    PackageAction::Remove,
                    &spec,
                    workspace.clone(),
                    ignore_scripts,
                    SaveType::Prod,
                )
                .await
                {
                    log_error(&e.to_string());
                    let _ = write_verbose_logs_to_file();
                    process::exit(1);
                }
            } else {
                return Err("Package specification is required for uninstall".into());
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
        Some(Commands::Update) => {
            if let Err(e) = update(false).await {
                log_error(&e.to_string());
                let _ = write_verbose_logs_to_file();
                process::exit(1);
            }
        }
        Some(Commands::Run { script }) => {
            if let Err(e) = cmd::run::run_script(&script).await {
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

    Ok(())
}
