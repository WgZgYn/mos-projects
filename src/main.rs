#![feature(const_format_args)]

// src/main.rs
use crate::command::Cli;
use crate::execute::parse_run;
use clap::Parser;
use log::LevelFilter;

mod command;
mod execute;
mod path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let cli = Cli::parse();
    parse_run(cli)
}
