use std::path::PathBuf;

use mako_core::clap;
use mako_core::clap::Parser;

use crate::config::Mode;

#[derive(Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub watch: bool,
    pub root: PathBuf,
    #[arg(long, default_value_t = Mode::Development, value_enum)]
    pub mode: Mode,
}
