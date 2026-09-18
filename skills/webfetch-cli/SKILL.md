---
name: webfetch-cli
description: Fetch web pages as Markdown, text, or JSON using the webfetch-cli Rust binary instead of the webfetch-mcp Python MCP server. Use this when you want a fast, dependency-free fetch without starting the MCP server. The CLI talks directly to the target URL and returns the same response schema as the MCP tools.
---

# Webfetch CLI

Invoke the `webfetch-cli` Rust binary to fetch web pages. It replaces the
Python MCP server's `fetch_readable`, `fetch_markdown`, `fetch_txt`, and
`fetch_json` tools with a single-command invocation — no server to start, no
MCP protocol.

## Binary location

```
/github/teo-mateo/ai-toolbox/mcp/webfetch-mcp/cli/target/release/webfetch-cli
```

If the binary does not exist, build it first:

```bash
cd /github/teo-mateo/ai-toolbox/mcp/webfetch-mcp/cli
cargo build --release
```

## Usage

> **Always pass `--json`** when an agent parses the output: the JSON return
> value is byte-for-byte identical to the MCP server's tool result, so it can
> be parsed the same way.

```bash
webfetch-cli fetch <URL> [--mode <mode>] [--max-length <n>] [--start-index <n>] [-H "Key: Value"]... [--json]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<URL>` | yes | HTTP(S) URL to fetch |
| `--mode` | no | `readable`, `markdown`, `txt`, or `json` (default: `markdown`) |
| `--max-length` | no | Maximum returned characters; 0 = unlimited (default: 12000) |
| `--start-index` | no | Character offset for pagination (default: 0) |
| `-H`, `--header` | no | Custom request header in `Key: Value` format (repeatable) |
| `--json` | no | Output raw JSON instead of human-readable text |

### Mode descriptions

| Mode | What it does |
|------|-------------|
| `readable` | Extract the main article content as Markdown (heuristic-based) |
| `markdown` | Convert the full HTML body to Markdown |
| `txt` | Return normalized plain text (whitespace collapsed) |
| `json` | Parse the response as JSON and pretty-print it |

### Examples

```bash
# Fetch as Markdown (default mode)
webfetch-cli fetch https://example.com/article

# Extract readable article content
webfetch-cli fetch https://news-site.com/story --mode readable

# Fetch plain text
webfetch-cli fetch https://example.com --mode txt

# Fetch and parse JSON API response
webfetch-cli fetch https://api.github.com/repos/rust-lang/rust --mode json

# Paginated fetch (next 1000 chars after position 5000)
webfetch-cli fetch https://example.com/long-page --max-length 1000 --start-index 5000

# With custom headers
webfetch-cli fetch https://example.com --mode markdown -H "Accept: text/html"

# JSON output for agent parsing
webfetch-cli fetch https://example.com --json
```

## Output

### Human-readable (default)
```
URL: https://example.com/article
Final URL: https://example.com/article
Type: text/html
Title: Example Article
Mode: markdown
---
# Example Article

This is the content of the article...
---
(truncated at 12000 chars, next: 12000)
```

### JSON (`--json`)
```json
{
  "ok": true,
  "mode": "markdown",
  "url": "https://example.com/article",
  "final_url": "https://example.com/article",
  "content_type": "text/html",
  "title": "Example Article",
  "text": "# Example Article\n\nThis is the content...",
  "truncated": true,
  "start_index": 0,
  "next_start_index": 12000
}
```

Error response:
```json
{
  "ok": false,
  "mode": "markdown",
  "url": "https://example.com",
  "error": "HTTP 404: Not Found"
}
```

This JSON format is **identical** to the MCP server's tool return values — an
agent can parse it the same way.

## Pagination

Use `--max-length` and `--start-index` to paginate through large pages:

1. Fetch with `--max-length 12000 --start-index 0`
2. If `truncated` is `true`, use `next_start_index` as the new `--start-index`
3. Repeat until `truncated` is `false`

## Security

The CLI enforces the same SSRF protections as the MCP server:

- Only `http://` and `https://` schemes allowed
- Blocks `localhost`, loopback, private, link-local, multicast, reserved, and
  unspecified IP addresses
- Resolves hostnames before each request to prevent DNS rebinding attacks
- Validates each redirect target before following it
- Enforces a maximum response size (default 5 MB)
- Maximum 8 redirects by default

## Configuration

The CLI reads its config from `config.toml` placed next to the binary
(at build time, `build.rs` copies `cli/config.toml` to `target/<profile>/config.toml`).
The config file is optional — if missing, all defaults are used.

Environment variables override the config file:

| Variable | Default | Purpose |
|----------|---------|---------|
| `WEBFETCH_MCP_DEFAULT_LIMIT` | `12000` | Default character limit |
| `WEBFETCH_MCP_MAX_RESPONSE_BYTES` | `5242880` | Max response body bytes (5 MB) |
| `WEBFETCH_MCP_TIMEOUT` | `15` | HTTP timeout in seconds |
| `WEBFETCH_MCP_MAX_REDIRECTS` | `8` | Max redirects to follow |

## When to use this skill

- **Instead of the MCP server** when you need to fetch a web page without
  starting the MCP server or managing a Python venv.
- **In scripts or pipelines** where a simple CLI invocation is cleaner than
  an MCP client session.
- **When the MCP server is unavailable** (e.g., Python venv not activated).
- **For quick one-off fetches** of pages, articles, or JSON APIs.

## When NOT to use this skill

- When you need the MCP protocol for a tool-calling workflow that expects
  the `fetch_readable`, `fetch_markdown`, `fetch_txt`, or `fetch_json` MCP
  tool names.
- When you need to fetch JavaScript-rendered pages (use `browser-fetch-mcp`
  instead, which uses Playwright).
- When fetching `localhost` or private network addresses (blocked by SSRF
  protection).
