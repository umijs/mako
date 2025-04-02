use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;

mod config;
mod error;

use config::Config;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    Set {
        key: String,
        value: String,
        #[arg(long)]
        global: bool,
    },
    Get {
        key: String,
        #[arg(long)]
        global: bool,
        #[arg(allow_hyphen_values = true)]
        #[arg(trailing_var_arg = true)]
        override_values: Vec<String>,
    },
    List {
        #[arg(long)]
        global: bool,
    },
}

// parse key val manullay
fn parse_key_val(s: &str) -> Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| anyhow::anyhow!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config { command } => match command {
            ConfigCommands::Set { key, value, global } => {
                let mut config = Config::load(global)?;
                config.set(&key, value, global)?;
                println!("Successfully set {} (global: {})", key, global);
            }
            ConfigCommands::Get {
                key,
                global,
                override_values,
            } => {
                let overrides: HashMap<String, String> = override_values
                    .iter()
                    .filter_map(|arg| {
                        if arg.starts_with("--") {
                            if let Ok((k, v)) = parse_key_val(&arg[2..]) {
                                return Some((k, v));
                            }
                        }
                        None
                    })
                    .collect();

                if let Some(value) = overrides.get(&key) {
                    println!("{}", value);
                } else {
                    let config = Config::load(global)?;
                    match config.get(&key)? {
                        Some(value) => println!("{}", value),
                        None => println!("No value set for {}", key),
                    }
                }
            }
            ConfigCommands::List { global } => {
                let config = Config::load(global)?;
                for (key, value) in config.list()? {
                    println!("{} = {}", key, value);
                }
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_global_config() {
        let home_dir = TempDir::new().unwrap();
        let global_config_path = home_dir.path().join(".utoo/config.toml");
        fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();

        fs::write(
            &global_config_path,
            r#"
            values = { "test.key" = "global_value" }
        "#,
        )
        .unwrap();

        std::env::set_var("HOME", home_dir.path());

        let config = Config::load(true).unwrap();
        assert_eq!(
            config.get("test.key").unwrap(),
            Some("global_value".to_string())
        );
    }

    #[test]
    fn test_local_override() {
        let home_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();

        let global_config_path = home_dir.path().join(".utoo/config.toml");
        fs::create_dir_all(global_config_path.parent().unwrap()).unwrap();
        fs::write(
            &global_config_path,
            r#"
            values = { "test.key" = "global_value" }
        "#,
        )
        .unwrap();

        let local_config_path = work_dir.path().join(".utoo.toml");
        fs::write(
            &local_config_path,
            r#"
            values = { "test.key" = "local_value" }
        "#,
        )
        .unwrap();

        std::env::set_var("HOME", home_dir.path());
        std::env::set_current_dir(work_dir.path()).unwrap();

        let config = Config::load(false).unwrap();
        assert_eq!(
            config.get("test.key").unwrap(),
            Some("local_value".to_string())
        );
    }

    #[test]
    fn test_cli_override() {
        let args = vec!["--test.key=cli_value"];
        let overrides: HashMap<String, String> = args
            .iter()
            .filter_map(|arg| {
                if arg.starts_with("--") {
                    if let Ok((k, v)) = parse_key_val(&arg[2..]) {
                        return Some((k, v));
                    }
                }
                None
            })
            .collect();

        assert_eq!(overrides.get("test.key").unwrap(), "cli_value");
    }
}
