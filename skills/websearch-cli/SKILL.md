---
name: websearch-cli
description: Search the web using the websearch-cli Rust binary instead of the websearch-mcp Python MCP server. Use this when you want a fast, dependency-free search without starting the MCP HTTP server. The CLI talks directly to SerpAPI or Exa and returns the same {title, url, snippet} results.
---

# Websearch CLI

Invoke the `websearch-cli` Rust binary to search the web. It replaces the
Python MCP server's `web_search` tool with a single-command invocation — no
server to start, no MCP protocol, no `websearch-client.sh` wrapper.

## Binary location

```
/github/teo-mateo/ai-toolbox/mcp/websearch-mcp/cli/target/release/websearch-cli
```

If the binary does not exist, build it first:

```bash
cd /github/teo-mateo/ai-toolbox/mcp/websearch-mcp/cli
cargo build --release
```

## Usage

```bash
websearch-cli search <QUERY> [--engine <engine>] [--limit <n>] [--json]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<QUERY>` | yes | Search query (natural-language phrase) |
| `--engine` | no | `serpapi` or `exa` (default from config) |
| `--limit` | no | Number of results, 1-20 (default 10) |
| `--json` | no | Output raw JSON instead of human-readable text |

### Examples

```bash
# Default engine, 10 results, human-readable
websearch-cli search "Rust async best practices 2025"

# Explicit engine, fewer results
websearch-cli search "Belgium news today" --engine serpapi --limit 5

# JSON output (for parsing by an agent)
websearch-cli search "best stocks this week" --json
```

## Output

### Human-readable (default)
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
```json
{
  "query": "Rust async best practices 2025",
  "results": [
    {
      "title": "Understanding async/await in Rust",
      "url": "https://blog.rust-lang.org/...",
      "snippet": "A comprehensive guide to async programming in Rust..."
    },
    ...
  ]
}
```

This JSON format is **identical** to the MCP server's `web_search` tool
return value — an agent can parse it the same way.

## Configuration

The CLI reads its config from `config.toml` placed next to the binary
(at build time, `build.rs` copies `cli/config.toml` to `target/<profile>/config.toml`).

Environment variables override the config file:

| Variable | Purpose |
|----------|---------|
| `SEARCH_ENGINE` | Override engine (`serpapi` or `exa`) |
| `SERPAPI_API_KEY` | Override SerpAPI key |
| `EXA_API_KEY` | Override Exa key |

## When to use this skill

- **Instead of the MCP server** when you need a quick search without
  starting the HTTP server or managing `websearch-client.sh`.
- **In scripts or pipelines** where a simple CLI invocation is cleaner than
  an MCP client session.
- **When the MCP server is unavailable** (e.g., port 7777 in use, Python
  venv not activated).

## When NOT to use this skill

- When you need the MCP protocol for a tool-calling workflow that expects
  the `web_search` MCP tool name.
- When the config file is missing and no env vars are set (the CLI will
  error out with a clear message about the missing API key).
