mod config;
mod output;
mod search;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "websearch-cli", version, about = "Web search CLI via SerpAPI or Exa")]
struct Cli {
    /// Output raw JSON instead of human-readable text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search the web
    Search {
        /// Search query (natural-language phrase)
        query: String,

        /// Search engine: serpapi or exa (default from config)
        #[arg(long)]
        engine: Option<String>,

        /// Number of results (1-20, default 10)
        #[arg(long, default_value_t = 10)]
        limit: u8,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Search { query, engine, limit } => {
            // Validate limit range
            if limit < 1 || limit > 20 {
                eprintln!("Error: --limit must be between 1 and 20");
                std::process::exit(1);
            }

            let config = config::load(engine)?;

            let q = query.trim();
            if q.is_empty() {
                eprintln!("Error: query must not be empty");
                std::process::exit(1);
            }

            let result = search::search(&config, q, limit as usize)?;

            if cli.json {
                output::print_json(&result);
            } else {
                output::print_human(&result);
            }
        }
    }

    Ok(())
}
