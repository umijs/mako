use crate::config::Mode;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub watch: bool,
    pub root: PathBuf,
    #[arg(long, default_value_t = Mode::Development, value_enum)]
    pub mode: Mode,
}
