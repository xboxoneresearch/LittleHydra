use clap::Parser;
use log::LevelFilter;

#[derive(Parser)]
#[command(name = "little-hydra")]
#[command(about = "A modular Windows process manager daemon")]
#[command(version)]
pub struct Cli {
    /// Log level verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// TCP log host (format: host:port)
    #[arg(long)]
    pub log_host: Option<String>,

    /// Path to config file (default: config.toml)
    #[arg(short = 'c', long = "config", default_value = "config.toml")]
    pub config: String,
}

impl Cli {
    pub fn get_log_level(&self) -> LevelFilter {
        match self.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            3.. => LevelFilter::Trace,
        }
    }
}
