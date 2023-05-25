use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub watch: bool,
    pub root: PathBuf,
}
