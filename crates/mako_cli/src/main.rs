use clap::{command, Parser};
use mako_bundler::config::Config;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    entry: Option<String>,
    #[arg(short, long, default_value = ".")]
    root: String,
}

fn join_paths(current: &Path, another: &Path) -> PathBuf {
    let joined = current.join(another);
    joined.canonicalize().expect("Failed to canonicalize path")
}

impl Into<Config> for Cli {
    fn into(&self) -> Config {
        let current_path = std::env::current_dir().expect("Failed to get current directory");
        let result = join_paths(&current_path, &Path::new(self.root.as_str()));

        let entry = match self.entry {
            Some(ref entry) => entry.clone(),
            None => "index.tsx".to_string(),
        };

        Config {
            entry: {
                let mut map = HashMap::new();
                map.insert("index".to_string(), entry);
                map
            },
            root: result.to_string_lossy().to_string(),
            externals: {
                let mut map = HashMap::new();
                map.insert("react".to_string(), "React".to_string());
                map.insert("react-dom/client".to_string(), "ReactDOM".to_string());
                map
            },
            ..Default::default()
        }
    }
}

fn main() {
    let cli = Cli::parse();
    mako_bundler::run_with_config(cli.into())
}
