use std::{collections::HashMap, path::PathBuf};

use clap::ValueEnum;

use futures::{channel::mpsc::channel, SinkExt, StreamExt};
use notify::{
    event::{DataChange, ModifyKind},
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use serde::Deserialize;
use swc_ecma_preset_env::Targets;

#[derive(Deserialize, Debug)]
pub struct OutputConfig {
    pub path: PathBuf,
}

#[derive(Deserialize, Debug)]
pub struct ResolveConfig {
    pub alias: HashMap<String, String>,
    pub extensions: Vec<String>,
}

#[derive(Deserialize, Debug, ValueEnum, Clone)]
pub enum Mode {
    #[serde(rename = "development")]
    Development,
    #[serde(rename = "production")]
    Production,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value().unwrap().get_name().fmt(f)
    }
}

macro_rules! named_unit_variant {
    ($variant:ident) => {
        pub mod $variant {
            pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = ();
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(concat!("\"", stringify!($variant), "\""))
                    }
                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        if value == stringify!($variant) {
                            Ok(())
                        } else {
                            Err(E::invalid_value(serde::de::Unexpected::Str(value), &self))
                        }
                    }
                }
                deserializer.deserialize_str(V)
            }
        }
    };
}

mod strings {
    named_unit_variant!(inline);
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum SourcemapConfig {
    Bool(bool),
    /// Generate inline sourcemap
    #[serde(with = "strings::inline")]
    Inline,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub entry: HashMap<String, PathBuf>,
    pub output: OutputConfig,
    pub resolve: ResolveConfig,
    pub mode: Mode,
    pub sourcemap: SourcemapConfig,
    pub externals: HashMap<String, String>,
    pub copy: Vec<String>,
    pub public_path: String,
    pub data_url_limit: usize,
    pub targets: Targets,
}

// pub struct CliConfig {}

const CONFIG_FILE: &str = "mako.config.json";
const DEFAULT_CONFIG: &str = r#"
{
    "entry": {},
    "output": { "path": "dist" },
    "resolve": { "alias": {}, "extensions": ["js", "jsx", "ts", "tsx"] },
    "mode": "development",
    "sourcemap": false,
    "externals": {},
    "copy": ["public"],
    "public_path": "/",
    "data_url_limit": 8192,
    "targets": { "chrome": 80 }
}
"#;

// TODO:
// - support .ts file
// - add Default impl
// - add test
// - add validation
// - rename sourcemap to devtool?

impl Config {
    pub fn new(root: &PathBuf) -> Result<Self, config::ConfigError> {
        let abs_config_file = root.join(CONFIG_FILE);
        let abs_config_file = abs_config_file.to_str().unwrap();
        let c = config::Config::builder()
            // default config
            .add_source(config::File::from_str(
                DEFAULT_CONFIG,
                config::FileFormat::Json,
            ))
            // user config
            .add_source(config::File::with_name(abs_config_file).required(false))
            // cli config
            .build()?;
        let mut ret = c.try_deserialize::<Config>();
        // normalize & check
        if let Ok(config) = &mut ret {
            if config.output.path.is_relative() {
                config.output.path = root.join(config.output.path.to_string_lossy().to_string());
            }

            if config.public_path != "runtime" && !config.public_path.ends_with('/') {
                panic!("public_path must end with '/' or be 'runtime'");
            }

            Config::config_node_polyfill(config);

            // let entry_length = cc.entry.len();
            // if entry_length != 1 {
            //     panic!(
            //         "Only one entry is allowed, but {} entries are found",
            //         entry_length
            //     );
            // }
        }
        ret
    }

    pub fn watch<T>(&self, root: &PathBuf, func: T)
    where
        T: Fn(),
    {
        futures::executor::block_on(async {
            self.watch_async(root, func).await;
        });
    }

    pub async fn watch_async<T>(&self, root: &PathBuf, func: T)
    where
        T: Fn(),
    {
        let (mut tx, mut rx) = channel(1);
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            notify::Config::default(),
        )
        .unwrap();
        let abs_config_file = root.join(CONFIG_FILE);
        watcher
            .watch(abs_config_file.as_path(), RecursiveMode::NonRecursive)
            .unwrap();
        while let Some(res) = rx.next().await {
            match res {
                Ok(event) => {
                    if let EventKind::Modify(ModifyKind::Data(DataChange::Any)) = event.kind {
                        println!("{:?}", event);
                        func();
                    }
                }
                Err(e) => {
                    println!("watch error: {:?}", e);
                }
            }
        }
    }
}
