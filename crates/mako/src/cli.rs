use std::path::PathBuf;

use clap::Parser;

use crate::config::Mode;

#[derive(Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub watch: bool,
    pub root: PathBuf,
    #[arg(long, default_value_t = Mode::Development, value_enum)]
    pub mode: Mode,
}
