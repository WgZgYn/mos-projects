use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(author = "wzy")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, default_value = "release", value_enum)]
    pub profile: Profile,

    #[arg(long)]
    pub boot_mode: Option<BootMode>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Profile {
    Debug,
    Release,
}
impl Profile {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    Run(Run),
    Build(Build),
}

#[derive(Copy, Clone, Eq, PartialEq, ValueEnum)]
pub enum BuildTarget {
    User,
    Kernel,
    Ramfs,
    All,
}

#[derive(Parser)]
pub struct Build {
    #[arg(long, default_value = "all", value_enum)]
    pub target: BuildTarget,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
pub enum BootMode {
    Uefi,
    Bios,
}

impl BootMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            BootMode::Uefi => "uefi",
            BootMode::Bios => "bios",
        }
    }
}

#[derive(Parser)]
pub struct Run {
    #[arg(long, default_value = "true")]
    pub map_ramfs_memory: bool,

    #[arg(long, default_value = "true")]
    pub build_if_updated: bool,
}
