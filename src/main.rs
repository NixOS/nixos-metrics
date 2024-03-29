use anyhow::Result;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use env_logger;
use nixos_metrics::{gtrends, netlify};

#[derive(Subcommand, Debug)]
enum Commands {
    /// Export netlify metrics, prints to stdout
    ScrapeNetlify(netlify::Cli),
    ProcessNetlify(netlify::process::Cli),
    ScrapeGtrends(gtrends::Cli),
    ProcessGtrends(gtrends::process::Cli),
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity,

    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    match &cli.command {
        Commands::ScrapeNetlify(cmd_args) => netlify::run(&cmd_args).await?,
        Commands::ProcessNetlify(cmd_args) => netlify::process::run(&cmd_args).await?,
        Commands::ScrapeGtrends(cmd_args) => gtrends::run(&cmd_args).await?,
        Commands::ProcessGtrends(cmd_args) => gtrends::process::run(&cmd_args).await?,
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {:?}", err);
        std::process::exit(1);
    }
}
