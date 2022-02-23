use anyhow::Result;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use log::SetLoggerError;
use nixos_metrics::netlify;
use simplelog::{ColorChoice, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};

/// Setup logging
fn setup_logger(verbose: &Verbosity) -> Result<(), SetLoggerError> {
    TermLogger::init(
        verbose
            .log_level()
            .map_or(LevelFilter::Error, |l| l.to_level_filter()),
        ConfigBuilder::new()
            .set_time_level(LevelFilter::Debug)
            .build(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
}

#[derive(Subcommand)]
enum Commands {
    /// Export netlify metrics, prints to stdout
    Netlify(netlify::Cli),
}

#[derive(Parser)]
#[clap(name = "nixos-metrics")]
#[clap(about = "A CLI for fetching various NixOS related metrics", long_about = None)]
struct Cli {
    #[clap(flatten)]
    verbose: Verbosity,

    #[clap(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn run() -> Result<()> {
    let args = Cli::parse();

    setup_logger(&args.verbose)?;

    match &args.command {
        Commands::Netlify(cmd_args) => netlify::run(&cmd_args).await,
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {:?}", err);
        std::process::exit(1);
    }
}
