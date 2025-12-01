// src/main.rs

use crate::command::Cli;
use crate::execute::parse_run;
use clap::Parser;

mod command;
mod execute;
mod path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();
    parse_run(cli)
}
