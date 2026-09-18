# webfetch-cli

A Rust CLI for fetching web pages as readable Markdown, Markdown, plain text,
or JSON. It is a direct reimplementation of the `webfetch-mcp` Python MCP
server's four `fetch_*` tools as a standalone command-line tool — no MCP
protocol or server required.

## Build

```bash
cd cli
cargo build --release
# binary at target/release/webfetch-cli
```

## Configuration

Settings are resolved from four sources, in increasing priority:
**default < config file < environment variable < CLI flag.**

### Config file (TOML, optional)

The config file lives **next to the binary** as `config.toml`. It is copied
there from the source `cli/config.toml` at build time. All keys are optional —
if the file is missing, defaults are used.

- Source config: `cli/config.toml` (gitignored)
- Template: `cli/config.toml.example` (committed)

```bash
cp config.toml.example config.toml   # optional overrides
cargo build --release                # copies config.toml next to the binary
```

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `WEBFETCH_MCP_DEFAULT_LIMIT` | `12000` | Default character limit |
| `WEBFETCH_MCP_MAX_RESPONSE_BYTES` | `5242880` | Max response body bytes |
| `WEBFETCH_MCP_TIMEOUT` | `15` | HTTP timeout in seconds |
| `WEBFETCH_MCP_MAX_REDIRECTS` | `8` | Max redirects to follow |

## Usage

```bash
webfetch-cli fetch <URL> [--mode <readable|markdown|txt|json>] [--max-length <n>] [--start-index <n>] [-H "Key: Value"]... [--json]
```

### Examples

```bash
# Fetch as Markdown (default mode)
webfetch-cli fetch https://example.com/article

# Extract readable article content
webfetch-cli fetch https://news-site.com/story --mode readable

# Fetch plain text
webfetch-cli fetch https://example.com --mode txt

# Fetch and parse a JSON API response
webfetch-cli fetch https://api.github.com/repos/rust-lang/rust --mode json

# Paginated fetch (next 1000 chars after position 5000)
webfetch-cli fetch https://example.com/long-page --max-length 1000 --start-index 5000

# With custom headers
webfetch-cli fetch https://example.com --mode markdown -H "Accept: text/html"

# JSON output for agent parsing
webfetch-cli fetch https://example.com --json
```

## Security

The CLI enforces the same SSRF protections as the MCP server:

- Only `http://` and `https://` schemes allowed
- Blocks `localhost`, loopback, private, link-local, multicast, reserved, and
  unspecified IP addresses
- Resolves hostnames before each request to prevent DNS-rebinding attacks
- Validates each redirect target before following it
- Enforces a maximum response size (default 5 MB)
- Maximum 8 redirects by default
