# Plan: Rust CLI for websearch-mcp

## Goal

Reimplement the `web_search` tool from the Python `websearch-mcp` MCP server as
a standalone Rust CLI. The CLI talks directly to SerpAPI or Exa (no MCP
protocol, no HTTP server). An AI-agent skill file (`websearch-cli.md`) in the
project root lets agents invoke this CLI instead of the MCP server.

## Source analysis (Python MCP server)

`src/websearchmcp/server.py` exposes one tool: `web_search(query, n_results)`.

| Backend | API call |
|---------|----------|
| SerpAPI | `GET https://serpapi.com/search.json?engine=google&q=<query>&num=<n>&api_key=<key>` |
| Exa | `POST https://api.exa.ai/search` with `Authorization: Bearer <key>` |

Both return `{title, url, snippet}` hits, deduplicated by URL. Title truncated
to 300 chars, snippet to 500 chars. Responses are logged to `logs/` as JSON.

Config from `.env`: `SEARCH_ENGINE` (default `serpapi`), `SERPAPI_API_KEY`,
`EXA_API_KEY`.

## CLI design

### Command surface (clap derive)

```
websearch-cli search <QUERY> [OPTIONS]
```

Single subcommand `search` mirrors the MCP tool. No nesting — the Python server
has only one tool.

| Argument | Required | Description |
|----------|----------|-------------|
| `<QUERY>` | yes | Search query (positional) |
| `--engine <e>` | no | `serpapi` or `exa` (default from config) |
| `--limit <n>` | no | Results 1–20 (default 10) |
| `--json` | no | Output raw JSON (global flag) |

`--json` is a global flag on the top-level `Cli` struct (matching the
fastmail-mcp pattern), so it works regardless of subcommand.

### Configuration

Three-tier resolution (lowest → highest priority):

1. **Config file** (`config.toml` beside the binary, copied at build time)
2. **Environment variables**
3. **CLI flags** (`--engine`)

This matches the fastmail-mcp pattern exactly.

#### Config file (TOML)

```toml
# websearch-cli configuration
engine = "exa"
serpapi_api_key = "..."
exa_api_key = "..."
```

No `default` / multi-account needed — this is a single-configuration tool.

#### Environment variables

| Variable | Overrides |
|----------|-----------|
| `SEARCH_ENGINE` | config `engine` |
| `SERPAPI_API_KEY` | config `serpapi_api_key` |
| `EXA_API_KEY` | config `exa_api_key` |

> **Important**: The env var names match the Python server's `.env` exactly
> (`SEARCH_ENGINE`, `SERPAPI_API_KEY`, `EXA_API_KEY`). This ensures agents
> sharing the same `.env` file can use either the MCP server or the CLI
> without configuration changes.

#### `.env` backward compatibility

The project's `.env` file (at the websearch-mcp project root) uses the same
names (`SEARCH_ENGINE`, `SERPAPI_API_KEY`, `EXA_API_KEY`). The CLI does **not**
read `.env` directly — it uses TOML + env vars. The skill file instructs agents
to either:

1. Use `config.toml` (recommended), or
2. Export env vars matching the CLI's names before running

This keeps the CLI independent of Python's `python-dotenv`.

### Crate layout

```
cli/
  Cargo.toml
  Cargo.lock
  build.rs                  # copies config.toml next to binary
  config.toml               # gitignored — your keys
  config.toml.example       # committed template
  PLAN.md
  README.md
  src/
    main.rs                 # clap CLI + dispatch (pattern match)
    config.rs               # TOML config + env var resolution
    search.rs               # SerpAPI + Exa HTTP calls
    output.rs               # human-readable / JSON output
    util.rs                 # response logging, filename sanitization
```

### Dependencies

| Crate | Features | Purpose |
|-------|----------|---------|
| `clap` | `derive`, `env` | CLI parsing |
| `reqwest` | `blocking`, `json`, `rustls-tls` | HTTP (no native-tls) |
| `serde` | `derive` | Serialization |
| `serde_json` | — | JSON I/O |
| `anyhow` | — | Error handling |
| `toml` | — | Config parsing |
| `chrono` | `clock` | Local-time timestamps (matches Python `datetime.now()`) |

No `base64` needed — no binary encoding.

### `Cargo.toml` (exact)

```toml
[package]
name = "websearch-cli"
version = "0.1.0"
edition = "2021"
description = "CLI for web search via SerpAPI or Exa"
license = "MIT"
build = "build.rs"

[dependencies]
clap = { version = "4", features = ["derive", "env"] }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["clock"] }

[profile.release]
lto = true
strip = true
```

### `build.rs` (exact)

Copies `config.toml` from source next to the compiled binary. Adapted from
fastmail-mcp's `build.rs`. If the source config does not exist, the build
proceeds without copying.

```rust
//! Build script: copy the source `config.toml` next to the compiled binary so
//! the CLI always finds its config beside itself.
//!
//! The source file (`CARGO_MANIFEST_DIR/config.toml`) is copied to
//! `target/<profile>/config.toml` on every build. If the source config does
//! not exist (e.g. a fresh clone), the build proceeds without copying.

use std::path::PathBuf;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR should be set by cargo");
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());

    let src = PathBuf::from(&manifest).join("config.toml");
    if !src.exists() {
        // No source config; the binary will use env vars or error at runtime.
        return;
    }

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(&manifest).join("target"));
    let dst = target_dir.join(&profile).join("config.toml");

    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::copy(&src, &dst).ok();

    println!("cargo:rerun-if-changed=config.toml");
}
```

### `config.toml.example` (exact)

```toml
# websearch-cli configuration
# Copy to config.toml and fill in your API keys.
# This file is copied next to the binary at build time.

engine = "exa"              # or "serpapi"
serpapi_api_key = "..."     # https://serpapi.com/
exa_api_key = "..."         # https://dashboard.exa.ai/
```

## API contracts

### SerpAPI

**Request:**
```
GET https://serpapi.com/search.json
  ?engine=google
  &q=<url-encoded query>
  &num=<1..20>
  &api_key=<key>
```

**Response** (JSON, relevant fields):
```json
{
  "organic_results": [
    {
      "title": "String",
      "link": "https://...",
      "snippet": "String (may be empty)"
    },
    ...
  ]
}
```

**Parsing:** Iterate `organic_results`. Extract `title` → `link` → `snippet`.
Skip entries with empty `link`. Deduplicate by `link`. Truncate title to 300,
snippet to 500. Stop after `n` hits.

### Exa

**Request:**
```
POST https://api.exa.ai/search
Authorization: Bearer <key>
Content-Type: application/json

{
  "query": "...",
  "type": "auto",
  "numResults": N,
  "contents": { "highlights": true }
}
```

**Response** (JSON, relevant fields):
```json
{
  "requestId": "string",
  "resolvedSearchType": "string",
  "results": [
    {
      "id": "string",
      "title": "string",
      "url": "https://...",
      "publishedDate": "string|null",
      "author": "string|null",
      "highlights": ["string", ...],
      "highlightScores": [number, ...]
    },
    ...
  ]
}
```

**Parsing:** Iterate `results`. Extract `title` → `url`. Snippet = first 3
`highlights` joined with space, truncated to 500 chars. If no highlights,
snippet is empty string. Skip entries with empty `url`. Deduplicate by `url`.
Stop after `n` hits.

Matches Python server's: `" ".join(highlights[:3])[:500]`.

## Module specifications

### `config.rs`

```rust
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    #[serde(default = "default_engine")]
    pub engine: String,
    #[serde(default)]
    pub serpapi_api_key: String,
    #[serde(default)]
    pub exa_api_key: String,
}

fn default_engine() -> String {
    "serpapi".to_string()
}

#[derive(Debug, Clone)]
pub struct Config {
    pub engine: String,      // "serpapi" or "exa"
    pub serpapi_api_key: String,
    pub exa_api_key: String,
}

/// Resolved config: CLI flag > env var > config file.
///
/// Environment variable names match the Python MCP server's .env:
/// `SEARCH_ENGINE`, `SERPAPI_API_KEY`, `EXA_API_KEY`.
pub fn load(engine_override: Option<String>) -> Result<Config> {
    // 1. Read config file (from beside the binary)
    let file = read_config_file()?;

    // 2. Resolve engine: CLI --engine > env SEARCH_ENGINE > config file
    let engine = engine_override
        .or_else(|| std::env::var("SEARCH_ENGINE").ok().map(|v| v.trim().to_lowercase()))
        .unwrap_or_else(|| file.engine.clone());

    // Validate engine
    if engine != "serpapi" && engine != "exa" {
        return Err(anyhow!(
            "Invalid engine '{}'. Must be 'serpapi' or 'exa'.",
            engine
        ));
    }

    // 3. Resolve API keys: env var > config file
    //    Env var names match the Python server's .env exactly.
    let serpapi_api_key = std::env::var("SERPAPI_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            (!file.serpapi_api_key.is_empty())
                .then_some(file.serpapi_api_key.clone())
        })
        .unwrap_or_default();

    let exa_api_key = std::env::var("EXA_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            (!file.exa_api_key.is_empty())
                .then_some(file.exa_api_key.clone())
        })
        .unwrap_or_default();

    Ok(Config {
        engine,
        serpapi_api_key,
        exa_api_key,
    })
}

fn resolve_config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let path = dir.join("config.toml");
    path.is_file().then_some(path)
}

fn read_config_file() -> Result<ConfigFile> {
    let path = resolve_config_path()
        .ok_or_else(|| anyhow!(
            "Config file not found. Place config.toml next to the binary (see config.toml.example)"
        ))?;

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file {}", path.display()))?;

    toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file {}", path.display()))?
}
```

### `search.rs`

```rust
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub query: String,
    pub results: Vec<SearchHit>,
}

const HTTP_TIMEOUT_SECS: u64 = 15;

fn http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .expect("reqwest client build")
}

pub fn search(config: &Config, query: &str, limit: usize) -> Result<SearchResult> {
    match config.engine.as_str() {
        "serpapi" => search_serpapi(config, query, limit),
        "exa" => search_exa(config, query, limit),
        _ => unreachable!(),
    }
}

// --- SerpAPI ---

#[derive(Deserialize, serde::Serialize)]
pub struct SerpapiResponse {
    #[serde(rename = "organic_results")]
    organic_results: Option<Vec<SerpapiHit>>,
}

#[derive(Deserialize, serde::Serialize)]
struct SerpapiHit {
    #[serde(default)]
    title: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    snippet: String,
}

fn search_serpapi(config: &Config, query: &str, limit: usize) -> Result<SearchResult> {
    let api_key = config.serpapi_api_key.clone();
    if api_key.is_empty() {
        return Err(anyhow!(
            "SERPAPI_API_KEY is not set — sign up at https://serpapi.com/ and set the SERPAPI_API_KEY environment variable or config.toml"
        ));
    }

    let client = http_client();
    let resp = client.get("https://serpapi.com/search.json")
        .query(&[
            ("engine", "google"),
            ("q", query),
            ("api_key", &api_key),
            ("num", &limit.to_string()),
        ])
        .send()
        .context("SerpAPI request failed")?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "SerpAPI returned HTTP {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    let data: SerpapiResponse = resp.json().context("Failed to parse SerpAPI response")?;

    // Log raw response (silently ignore failures)
    crate::util::log_serpapi_response(query, limit, &data);

    let raw = data.organic_results.unwrap_or_default();
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for r in raw {
        if r.link.is_empty() || seen_urls.contains(&r.link) {
            continue;
        }
        seen_urls.insert(r.link.clone());
        hits.push(SearchHit {
            title: r.title.chars().take(300).collect(),
            url: r.link,
            snippet: r.snippet.chars().take(500).collect(),
        });
        if hits.len() >= limit {
            break;
        }
    }

    Ok(SearchResult {
        query: query.to_string(),
        results: hits,
    })
}

// --- Exa ---

#[derive(Deserialize, serde::Serialize)]
pub struct ExaResponse {
    #[allow(dead_code)]
    pub request_id: Option<String>,
    #[allow(dead_code)]
    pub resolved_search_type: Option<String>,
    pub results: Option<Vec<ExaHit>>,
}

#[derive(Deserialize, serde::Serialize)]
struct ExaHit {
    #[allow(dead_code)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[allow(dead_code)]
    pub published_date: Option<String>,
    #[allow(dead_code)]
    pub author: Option<String>,
    pub highlights: Option<Vec<String>>,
    #[allow(dead_code)]
    pub highlight_scores: Option<Vec<f64>>,
}

#[derive(serde::Serialize)]
struct ExaRequest {
    pub query: String,
    #[serde(rename = "type")]
    pub search_type: String,
    #[serde(rename = "numResults")]
    pub num_results: usize,
    pub contents: ExaContents,
}

#[derive(serde::Serialize)]
struct ExaContents {
    pub highlights: bool,
}

fn search_exa(config: &Config, query: &str, limit: usize) -> Result<SearchResult> {
    let api_key = config.exa_api_key.clone();
    if api_key.is_empty() {
        return Err(anyhow!(
            "EXA_API_KEY is not set — sign up at https://dashboard.exa.ai/ and set the EXA_API_KEY environment variable or config.toml"
        ));
    }

    let client = http_client();
    let body = ExaRequest {
        query: query.to_string(),
        search_type: "auto".to_string(),
        num_results: limit,
        contents: ExaContents { highlights: true },
    };

    let resp = client.post("https://api.exa.ai/search")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .context("Exa request failed")?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Exa returned HTTP {}: {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ));
    }

    let data: ExaResponse = resp.json().context("Failed to parse Exa response")?;

    // Log raw response (silently ignore failures)
    crate::util::log_exa_response(query, &data);

    let raw = data.results.unwrap_or_default();
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for r in raw {
        if r.url.is_empty() || seen_urls.contains(&r.url) {
            continue;
        }
        seen_urls.insert(r.url.clone());

        let snippet = r.highlights
            .map(|h| h.iter().take(3).cloned().collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect();

        hits.push(SearchHit {
            title: r.title.chars().take(300).collect(),
            url: r.url,
            snippet,
        });
        if hits.len() >= limit {
            break;
        }
    }

    Ok(SearchResult {
        query: query.to_string(),
        results: hits,
    })
}
```

### `output.rs`

```rust
use crate::search::SearchResult;

/// Print human-readable output (matches websearch-client.sh format).
pub fn print_human(result: &SearchResult) {
    println!("Query: {}", result.query);
    println!("Results: {}", result.results.len());
    println!();
    for (i, hit) in result.results.iter().enumerate(1) {
        println!("{}. {}", i, hit.title);
        println!("   {}", hit.url);
        println!("   {}", hit.snippet);
        if i < result.results.len() {
            println!();
        }
    }
}

/// Print JSON output (matches MCP server's SearchResult.to_dict()).
pub fn print_json(result: &SearchResult) {
    let output = serde_json::json!({
        "query": result.query,
        "results": result.results.iter().map(|h| {
            serde_json::json!({
                "title": h.title,
                "url": h.url,
                "snippet": h.snippet,
            })
        }).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
```

### `util.rs`

```rust
use std::io::Write;

use crate::search::{ExaResponse, SerpapiResponse};

/// Sanitize a query string for use in filenames.
fn sanitize_filename(query: &str) -> String {
    let sanitized = query
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    if sanitized.is_empty() {
        "untitled".to_string()
    } else {
        sanitized
    }
}

/// Resolve the log directory.
///
/// The Python server logs to `logs/` at the project root. The CLI binary lives
/// at `<project>/cli/target/<profile>/websearch-cli`. We walk up from the
/// binary directory looking for a `logs/` directory, then fall back to
/// `logs/` relative to CWD.
fn resolve_log_dir() -> Option<std::path::PathBuf> {
    // Walk up from the binary looking for a `logs/` directory.
    if let Ok(exe) = std::env::current_exe() {
        let mut current = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..10 {
            match current {
                Some(ref dir) if dir.join("logs").is_dir() => return Some(dir.join("logs")),
                Some(dir) => current = dir.parent().map(|p| p.to_path_buf()),
                None => break,
            }
        }
    }
    // Fallback: logs/ relative to CWD
    std::env::current_dir().ok().map(|d| d.join("logs"))
}

/// Log a SerpAPI response to `logs/{timestamp}_{engine}_{query}.json`.
/// Silently ignores all errors.
pub fn log_serpapi_response(query: &str, limit: usize, data: &SerpapiResponse) {
    let log_dir = match resolve_log_dir() {
        Some(d) => d,
        None => return,
    };

    let ts = format_timestamp();
    let safe_query = sanitize_filename(query);
    let filename = format!("{}_google_{}.json", ts, safe_query);

    let log_entry = serde_json::json!({
        "timestamp": ts,
        "query": query,
        "params": {
            "engine": "google",
            "num": limit,
        },
        "response": data,
    });

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(log_dir.join(&filename))
    {
        let _ = writeln!(f, "{}", serde_json::to_string_pretty(&log_entry).unwrap_or_default());
    }
}

/// Log an Exa response to `logs/{timestamp}_exa_{query}.json`.
/// Silently ignores all errors.
pub fn log_exa_response(query: &str, data: &ExaResponse) {
    let log_dir = match resolve_log_dir() {
        Some(d) => d,
        None => return,
    };

    let ts = format_timestamp();
    let safe_query = sanitize_filename(query);
    let filename = format!("{}_exa_{}.json", ts, safe_query);

    let log_entry = serde_json::json!({
        "timestamp": ts,
        "query": query,
        "response": data,
    });

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(log_dir.join(&filename))
    {
        let _ = writeln!(f, "{}", serde_json::to_string_pretty(&log_entry).unwrap_or_default());
    }
}

/// Format current time as `YYYY-MM-DDTHH-MM-SS` matching the Python server's
/// `datetime.datetime.now().strftime("%Y-%m-%dT%H-%M-%S")` (local time).
fn format_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}
```

### `main.rs`

Uses idiomatic pattern matching for dispatch (matching the fastmail-mcp
reference pattern), not accessor methods on the Command enum.

```rust
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
```

## Output format

### Human-readable (default)

Matches `websearch-client.sh` output:

```
Query: Rust async best practices 2025
Results: 10

1. Understanding async/await in Rust
   https://blog.rust-lang.org/...
   A comprehensive guide to async programming in Rust, covering...

2. Tokio documentation
   https://tokio.rs/...
   Tokio is an asynchronous runtime for the Rust programming language...
```

### JSON (`--json`)

Matches MCP server's `SearchResult.to_dict()` exactly:

```json
{
  "query": "Rust async best practices 2025",
  "results": [
    {
      "title": "Understanding async/await in Rust",
      "url": "https://blog.rust-lang.org/...",
      "snippet": "A comprehensive guide to async programming in Rust..."
    }
  ]
}
```

## Logging

Mirrors the Python server's `_log_serpapi_response` / `_log_exa_response`:

- Each search response saved as JSON in `logs/` (project root)
- Filename: `{timestamp}_{engine}_{sanitized_query}.json`
- Content: `{timestamp, query, params (sans api_key), response}`
- Silently ignored on failure — never causes CLI to fail
- Timestamp uses **local time** via `chrono::Local::now()` (matches Python
  `datetime.datetime.now()`)

## README.md

A `cli/README.md` will be created (matching the fastmail-mcp reference pattern):

```markdown
# websearch-cli

A Rust CLI for web search via SerpAPI or Exa. It is a direct reimplementation
of the `websearch-mcp` Python MCP server's `web_search` tool as a standalone
command-line tool — no MCP protocol or server required.

## Build

```bash
cd cli
cargo build --release
# binary at target/release/websearch-cli
```

## Configuration

Settings are resolved from three sources, in increasing priority:
**config file < environment variable < CLI flag.**

### Config file (TOML)

The config file lives **next to the binary** as `config.toml`. It is copied
there from the source `cli/config.toml` at build time.

- Source config: `cli/config.toml` (gitignored — contains your keys)
- Template: `cli/config.toml.example` (committed — copy it to `config.toml`)

```bash
cp config.toml.example config.toml   # fill in your keys
cargo build --release                # copies config.toml next to the binary
```

### Environment variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `SEARCH_ENGINE` | no | Search engine: `serpapi` or `exa` (default `serpapi`). Overrides config file. |
| `SERPAPI_API_KEY` | no* | SerpAPI key. Overrides config file. *Required if engine is `serpapi`. |
| `EXA_API_KEY` | no* | Exa API key. Overrides config file. *Required if engine is `exa`. |

## Usage

```bash
websearch-cli search <QUERY> [--engine <engine>] [--limit <n>] [--json]
```

### Examples

```bash
# Default engine, 10 results, human-readable
websearch-cli search "Rust async best practices 2025"

# Explicit engine, fewer results
websearch-cli search "Belgium news today" --engine serpapi --limit 5

# JSON output (for parsing by an agent)
websearch-cli search "best stocks this week" --json
```

## Notes

- `--json` output matches the MCP server's `web_search` return value exactly.
- API keys are never logged; only the sanitized params appear in log files.
- Responses are logged to `logs/` at the project root (same as the MCP server).
```

## .gitignore update

Add the following lines to
`/github/teo-mateo/ai-toolbox/mcp/websearch-mcp/.gitignore`:

```
# Rust CLI
cli/config.toml
cli/target/
```

## Skill file

The skill file `websearch-cli.md` already exists at the project root
(`/github/teo-mateo/ai-toolbox/mcp/websearch-mcp/websearch-cli.md`).

It covers:
- Binary location and build instructions
- Usage with all flags
- Output format (human + JSON)
- Configuration
- When to use / when not to use

**The skill should always use `--json`** for agent parsing, since the JSON
output matches the MCP server's `SearchResult.to_dict()` exactly.

> The existing skill file references `WEBSEARCH_ENGINE` as the env var name.
> It should be updated to `SEARCH_ENGINE` to match the refined plan.

## Differences vs. the MCP server

| Aspect | MCP server | CLI |
|--------|-----------|-----|
| Protocol | MCP (Streamable HTTP) | None (plain CLI) |
| Config | `.env` (python-dotenv) | `config.toml` + env vars |
| Output | JSON (tool result) | Human-readable or `--json` |
| Invocation | MCP client session | Single command |
| Engine env var | `SEARCH_ENGINE` | `SEARCH_ENGINE` (same) |
| Auth key env vars | `SERPAPI_API_KEY`, `EXA_API_KEY` | Same |
| Error handling | Raises Python exceptions | anyhow + non-zero exit |
| Timeout | 15s (httpx) | 15s (reqwest) |
| Dedup + truncation | Yes | Yes (identical) |
| Logging | `logs/` at project root | `logs/` at project root |
| Timestamp | Local time (`datetime.now()`) | Local time (`chrono::Local`) |

## Implementation steps

1. **Scaffold** — Write `Cargo.toml`, `build.rs`, `config.toml.example`,
   `README.md`.
2. **`config.rs`** — TOML config file loading + env var resolution
   (`SEARCH_ENGINE`, `SERPAPI_API_KEY`, `EXA_API_KEY`).
3. **`search.rs`** — SerpAPI HTTP call (GET, parse `organic_results`).
4. **`search.rs`** — Exa HTTP call (POST, parse results + highlights).
5. **`output.rs`** — Human-readable table + `--json` output.
6. **`util.rs`** — Response logging to `logs/` with local-time timestamps.
7. **`main.rs`** — Wire up clap subcommand + dispatch (pattern match).
8. **Update `.gitignore`** — Add `cli/config.toml`, `cli/target/` to
   `mcp/websearch-mcp/.gitignore`.
9. **Update skill file** — Fix env var name from `WEBSEARCH_ENGINE` to
   `SEARCH_ENGINE` in `websearch-cli.md`.
10. **Build** — `cargo build --release`.
11. **Smoke-test** — Run against both engines with real API keys.

## Verification

- [ ] `cargo build --release` compiles with zero warnings
- [ ] `websearch-cli search "test query" --engine serpapi` returns results
- [ ] `websearch-cli search "test query" --engine exa` returns results
- [ ] `websearch-cli search "test query" --json` produces valid JSON
- [ ] JSON output matches MCP server's `SearchResult.to_dict()` schema
- [ ] Response is logged to `logs/` as JSON with local-time timestamp
- [ ] Missing API key produces clear error message
- [ ] Empty query is rejected
- [ ] `--limit 0` and `--limit 21` are rejected
- [ ] `--engine invalid` is rejected
- [ ] Deduplication works (duplicate URLs removed)
- [ ] Title truncated to 300 chars, snippet to 500 chars
- [ ] `SEARCH_ENGINE` env var overrides config file engine
- [ ] `SERPAPI_API_KEY` / `EXA_API_KEY` env vars override config file keys
- [ ] Skill file `websearch-cli.md` references correct env var names

## Status: PLANNED
