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
