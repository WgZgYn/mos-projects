#![feature(const_format_args)]

// src/main.rs
use crate::command::Cli;
use crate::execute::parse_run;
use clap::Parser;
use log::{LevelFilter, info};

mod command;
mod execute;
mod path;

/// FIXME: fs is not sync with build, sometime it's need to run twice.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let cli = Cli::parse();
    parse_run(cli)
}
