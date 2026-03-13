use std::env::current_dir;
use clap::Parser;
use log::LevelFilter;
use ramfs_builder::build_ramfs;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(long)]
    root: String,
    #[arg(short, long)]
    out: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let cli = Cli::parse();
    dbg!(&cli);
    dbg!(&current_dir());
    build_ramfs(&cli.root, &cli.out)?;
    Ok(())
}
