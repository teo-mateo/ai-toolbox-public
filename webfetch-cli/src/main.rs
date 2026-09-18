mod config;
mod fetch;
mod net;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "webfetch-cli", version, about = "Fetch web pages as Markdown, text, or JSON")]
struct Cli {
    /// Output raw JSON instead of human-readable text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch a web page
    Fetch {
        /// HTTP(S) URL to fetch
        url: String,

        /// Conversion mode: readable, markdown, txt, or json
        #[arg(long, value_enum, default_value_t = fetch::Mode::Markdown)]
        mode: fetch::Mode,

        /// Maximum returned characters (0 = unlimited)
        #[arg(long)]
        max_length: Option<usize>,

        /// Character offset for pagination
        #[arg(long, default_value_t = 0)]
        start_index: usize,

        /// Custom request header in "Key: Value" format (repeatable)
        #[arg(short = 'H', long = "header")]
        header: Vec<String>,
    },
}

fn parse_header(s: &str) -> Result<(String, String)> {
    let idx = s
        .find(':')
        .ok_or_else(|| anyhow::anyhow!("Invalid header '{}': expected 'Key: Value'", s))?;
    Ok((s[..idx].trim().to_string(), s[idx + 1..].trim().to_string()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Fetch { url, mode, max_length, start_index, header } => {
            let config = config::load();

            // Resolve max_length: CLI flag > env > config > default.
            let max_length = max_length.unwrap_or(config.default_limit);

            let headers: Vec<(String, String)> = header
                .iter()
                .map(|h| parse_header(h))
                .collect::<Result<Vec<_>>>()?;

            let mode_str = mode.as_str();

            // Fetch + convert; on error emit the server-compatible error shape.
            let outcome = (|| -> Result<()> {
                let result = fetch::fetch(&config, &url, &headers)?;
                let (text, title) = fetch::convert(&result, mode)?;
                let sliced = fetch::slice_text(&text, max_length, start_index);
                if cli.json {
                    let value = fetch::build_response(&result, title.clone(), &sliced, start_index, mode_str);
                    output::print_json(&value);
                } else {
                    output::print_human(&result, &title, mode_str, &sliced, max_length);
                }
                Ok(())
            })();

            match outcome {
                Ok(()) => {}
                Err(e) => {
                    let err = fetch::error_response(e.to_string(), mode_str, &url);
                    if cli.json {
                        output::print_json(&err);
                    } else {
                        eprintln!("Error: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
