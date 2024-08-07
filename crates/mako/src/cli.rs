use std::path::PathBuf;

use clap;
use clap::builder::TypedValueParser;
use clap::Parser;

use crate::config::Mode;

#[derive(Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub watch: bool,
    pub root: PathBuf,
    #[arg(long, default_value_t = Mode::Development,
        value_parser = clap::builder::PossibleValuesParser::new(["production", "prod", "p", "development","dev"])
                .map(|s|{
                    match s.as_str() {
                        "production" | "prod" | "p" => Mode::Production,
                        "development" | "dev" => Mode::Development,
                        _ => unreachable!()
                    }
                })
    )]
    pub mode: Mode,
}
