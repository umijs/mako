mod utils;

use crate::utils::start_watch;
use clap::Parser;
use mako_bundler::compiler::Compiler;
use mako_bundler::config;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct MakoCLI {
    #[arg(short, long, default_value_t = false)]
    watch: bool,
    root: PathBuf,
}

#[tokio::main]
async fn main() {
    let cli: MakoCLI = MakoCLI::parse();

    // config
    let root = std::env::current_dir().unwrap().join(cli.root.as_path());

    let mut config = config::Config {
        root,
        externals: maplit::hashmap! {
            "stream".to_string() => "stream".to_string()
        },
        entry: maplit::hashmap! {
            "index".to_string() => "index.tsx".to_string().into()
        },
        ..Default::default()
    };

    config.normalize();

    // compiler_origin::run_compiler(config);
    let root = config.root.clone();
    let mut compiler = Compiler::new(config);
    compiler.run();

    println!("✅ DONE");

    if cli.watch {
        start_watch(&root, &mut compiler);
    }
}
