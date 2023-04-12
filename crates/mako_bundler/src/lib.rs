#![feature(box_patterns)]

use std::{collections::HashMap, path::PathBuf, str::FromStr};

use compiler::Compiler;

pub(crate) mod build;
pub(crate) mod compiler;
pub(crate) mod config;
pub(crate) mod context;
pub(crate) mod generate;
pub(crate) mod module;
pub(crate) mod module_graph;

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        panic!("Please specify the root directory of the project");
    }

    // config
    let mut config = config::Config::default();

    // externals
    config
        .externals
        .insert("react".to_string(), "React".to_string());
    config
        .externals
        .insert("react-dom/client".to_string(), "ReactDOM".to_string());

    // root
    config.root = std::env::current_dir()
        .unwrap()
        .join(&args[1])
        .to_string_lossy()
        .to_string();

    // output
    config.output.path = PathBuf::from_str(&config.root)
        .unwrap()
        .join(config.output.path)
        .to_string_lossy()
        .to_string();

    // entry
    config.entry = {
        let mut entry = HashMap::new();
        entry.insert("index".to_string(), "index.tsx".to_string());
        entry
    };
    // only one entry is allowed
    let entry_length = config.entry.len();
    if entry_length != 1 {
        panic!(
            "Only one entry is allowed, but {} entries are found",
            entry_length
        );
    }

    // compiler_origin::run_compiler(config);
    let mut compiler = Compiler::new(config);
    compiler.run();
}
