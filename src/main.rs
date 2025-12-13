#![feature(const_format_args)]

// src/main.rs
use crate::command::Cli;
use crate::execute::parse_run;
use clap::Parser;

mod command;
mod execute;
mod path;

/// FIXME: fs is not sync with build, sometime it's need to run twice.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();
    parse_run(cli)
}
