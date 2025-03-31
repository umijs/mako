use std::path::PathBuf;

use clap::{Args, Parser};

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
pub enum CliArgs {
    Build(Options),
    Dev(Options),
}

#[derive(Debug, Args, Clone)]
pub struct Options {
    #[clap(short, long, value_parser)]
    pub project: Option<PathBuf>,

    /// The root directory of the project. Nothing outside of this directory can
    /// be accessed. e. g. the monorepo root.
    /// If no directory is provided, `dir` will be used.
    #[clap(long, value_parser)]
    pub root: Option<PathBuf>,

    /// minify build output.
    #[clap(long)]
    pub minify: bool,
}
