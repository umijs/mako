use std::process;

use clap::{App, SubCommand};
use cmd::deps::build_deps;
use cmd::install::install;
use cmd::rebuild::rebuild;
use cmd::{clean::clean, deps::build_workspace};
use helper::auto_update::init_auto_update;
use util::config::set_registry;
use util::logger::{log_error, log_info, log_warning, set_verbose, write_verbose_logs_to_file};

mod cmd;
mod helper;
mod model;
mod service;
mod util;

#[tokio::main]
async fn main() {
    unsafe {
        libc::umask(0o00);
    }

    // Ensure the version is up to date, weak dependency
    if let Err(_e) = init_auto_update().await {
        log_warning(&format!("Auto update cancelled"));
    }

    let app = App::new("🌖 utoo")
        .version(env!("CARGO_PKG_VERSION"))
        .about("/juːtuː/ Unified Toolchain: Open & Optimized")
        .arg(clap::Arg::with_name("ignore-scripts")
            .long("ignore-scripts")
            .help("Install, skip running npm scripts during installation"))
        .arg(clap::Arg::with_name("verbose")
            .long("verbose")
            .short('V')
            .global(true)
            .help("Verbose, show verbose log"))
        .arg(clap::Arg::with_name("version")
            .short('v')
            .long("version")
            .help("Print version info and exit"))
        .arg(clap::Arg::with_name("registry")
            .long("registry")
            .global(true)
            .takes_value(true)
            .default_value("https://registry.npmmirror.com")
            .help("Specify npm registry URL for dependency resolution and installation"))
        .subcommand(
            SubCommand::with_name("install")
                .alias("i")
                .about("Go to install deps (alias: i)")
                .arg(clap::Arg::with_name("ignore-scripts")
                    .long("ignore-scripts")
                    .help("Skip running npm scripts during installation"))
        )
        .subcommand(
            SubCommand::with_name("rebuild")
                .alias("rb")
                .about("Do rebuild deps hook scripts (alias: r)")
        )
        // .subcommand(
        //     SubCommand::with_name("update")
        //         .alias("up")
        //         .about("Update node_modules in current project (alias: up)")
        //         .arg(clap::Arg::with_name("ignore-scripts")
        //             .long("ignore-scripts")
        //             .help("Skip running npm scripts during installation"))
        // )

        .subcommand(
            SubCommand::with_name("clean")
                .alias("c")
                .about("Clean store in ~/.cache/nm (alias: c)")
                .arg(clap::Arg::with_name("pattern")
                    .help("Package name and version (e.g. markdown-it@2.0.0, markdown-*@*, *@2.0.0)")
                    .default_value("*")
                    .required(false))
        )
        .subcommand(
            SubCommand::with_name("deps")
                .alias("d")
                .about("Generate package-lock.json file (alias: d), expiremental feature")
                .arg(clap::Arg::with_name("workspace-only")
                    .long("workspace-only")
                    .help("Only build workspace packages"))
        );

    // extend app
    let (extended_app, scripts_help) = cmd::pkg::extend_app(app);
    let app_with_help = extended_app.after_help(&*scripts_help);
    let matches = app_with_help.get_matches();

    // global verbose
    set_verbose(matches.is_present("verbose"));

    // global registry
    set_registry(matches.value_of("registry").unwrap());

    // load package.json
    if let Some(result) = cmd::pkg::handle_command(&matches) {
        if let Err(e) = result {
            log_error(&e);
        }
        return;
    }

    // inner command
    match matches.subcommand_name() {
        // Some("update") => {
        //     if let Some(update_matches) = matches.subcommand_matches("update") {
        //         if let Err(e) = update().await {
        //             log_error(&e.to_string());
        //             process::exit(1);
        //         }
        //         if let Err(e) = install(update_matches.is_present("ignore-scripts")).await {
        //             log_error(&e.to_string());
        //             let _ = write_verbose_logs_to_file();
        //             process::exit(1);
        //         }
        //     }
        // }
        Some("clean") => {
            if let Some(clean_matches) = matches.subcommand_matches("clean") {
                if let Err(e) = clean(clean_matches.value_of("pattern").unwrap()).await {
                    log_error(&e.to_string());
                    let _ = write_verbose_logs_to_file();
                    process::exit(1);
                }
            }
        }
        Some("install") => {
            if let Some(install_matches) = matches.subcommand_matches("install") {
                if let Err(e) = install(install_matches.is_present("ignore-scripts")).await {
                    log_error(&e.to_string());
                    let _ = write_verbose_logs_to_file();
                    process::exit(1);
                }
            }
        }
        Some("rebuild") => {
            log_info("Executing dependency hook scripts and creating node_modules/.bin links");
            if let Err(e) = rebuild().await {
                log_error(&e);
                process::exit(1);
            }
            log_info("💫 All dependencies rebuild completed");
        }
        Some("deps") => {
            if let Some(deps_matches) = matches.subcommand_matches("deps") {
                let result = if deps_matches.is_present("workspace-only") {
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
        }
        _ => {
            // install by default
            if let Err(e) = install(matches.is_present("ignore-scripts")).await {
                log_error(&e.to_string());
                let _ = write_verbose_logs_to_file();
                process::exit(1);
            }
        }
    }
}
