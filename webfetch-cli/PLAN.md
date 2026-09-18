# Plan: Rust CLI for webfetch-mcp

## Goal

Reimplement the four `fetch_*` tools from the Python `webfetch-mcp` MCP server
as a single standalone Rust CLI. The CLI talks directly to the target URL (no
MCP protocol, no HTTP server) while preserving the same response schema, the
same SSRF protections, and the same pagination semantics as the server. An
AI-agent skill file (`webfetch-cli.md`) lets agents invoke this CLI instead of
the MCP server.

This plan mirrors the completed `websearch-cli` project in
`mcp/websearch-mcp/cli` (which itself follows the `fastmail-mcp/cli` reference
pattern): clap-derive CLI, `config.toml` beside the binary copied at build
time, modular `src/` layout, and `--json` output that matches the MCP server's
tool return value exactly.

## Source analysis (Python MCP server)

`src/webfetchmcp/server.py` exposes four tools, all with the same parameter
shape and return schema:

| Tool | Mode | Behavior |
|------|------|----------|
| `fetch_readable` | `readable` | Extract main article content as Markdown (readability-lxml `Document`) |
| `fetch_markdown` | `markdown` | Convert cleaned HTML `<body>` to Markdown (markdownify, ATX headings) |
| `fetch_txt` | `txt` | Return normalized plain text (whitespace collapsed) |
| `fetch_json` | `json` | Parse response as JSON and pretty-print |

### Common parameters (all four tools)

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `url` | string | yes | — | HTTP(S) URL to fetch |
| `headers` | object | no | `{}` | Optional request headers (merged over `User-Agent`) |
| `max_length` | integer | no | `12000` | Max returned characters; `0` = unlimited |
| `start_index` | integer | no | `0` | Character offset for pagination |

### Return schema (all four tools)

Success:
```json
{
  "ok": true,
  "mode": "readable",
  "url": "https://...",
  "final_url": "https://...",
  "content_type": "text/html",
  "title": "Example Article",
  "text": "# Example Article\n\n...",
  "truncated": false,
  "start_index": 0,
  "next_start_index": null
}
```

Failure (any exception is caught and returned as JSON, never raised):
```json
{
  "ok": false,
  "mode": "readable",
  "url": "https://...",
  "error": "Blocked private/local address '127.0.0.1'."
}
```

### Safety (SSRF) — must be replicated exactly

`_validate_url` + `_validate_resolved_ips`:

1. Only `http://` and `https://` schemes allowed.
2. URL must include a hostname.
3. Literal IP hosts are checked directly; `localhost` is blocked.
4. Domain hosts are resolved via `socket.getaddrinfo` and **every** resolved IP
   is checked (mitigates DNS rebinding).
5. Blocked IP classes (Python `ipaddress._is_blocked_ip`):
   `is_private`, `is_loopback`, `is_link_local`, `is_multicast`, `is_reserved`,
   `is_unspecified`.
6. Redirects are followed manually; **each redirect target is re-validated**
   before the request is made.
7. Max response body bytes enforced (default 5 MB).
8. Max redirects (default 8).

### Environment variables (server)

| Variable | Default | Description |
|----------|---------|-------------|
| `WEBFETCH_MCP_DEFAULT_LIMIT` | `12000` | Default returned character limit |
| `WEBFETCH_MCP_MAX_RESPONSE_BYTES` | `5242880` | Maximum response body bytes |
| `WEBFETCH_MCP_TIMEOUT` | `15` | HTTP timeout in seconds |
| `WEBFETCH_MCP_MAX_REDIRECTS` | `8` | Maximum redirects to follow |

The server sends a fixed `User-Agent` (Chrome 120 on Linux x86_64).

## CLI design

### Command surface (clap derive)

```
webfetch-cli fetch <URL> [OPTIONS]
```

A single `fetch` subcommand consolidates the server's four tools via a `--mode`
flag (the server has no subcommand nesting; the CLI mirrors `websearch-cli`'s
single `search` subcommand pattern).

| Argument | Required | Description |
|----------|----------|-------------|
| `<URL>` | yes | HTTP(S) URL to fetch (positional) |
| `--mode <m>` | no | `readable`, `markdown`, `txt`, or `json` (default `markdown`) |
| `--max-length <n>` | no | Max returned chars; `0` = unlimited (default from config/env) |
| `--start-index <n>` | no | Character offset for pagination (default `0`) |
| `-H, --header <"Key: Value">` | no | Custom request header, repeatable |
| `--json` | no | Output raw JSON (global flag) |

`--json` is a global flag on the top-level `Cli` struct (matching the
fastmail/websearch pattern), so it works regardless of subcommand.

### Configuration

Three-tier resolution (lowest → highest priority):

1. **Defaults** (12000 / 5242880 / 15 / 8 / fixed User-Agent)
2. **Config file** (`config.toml` beside the binary, copied at build time)
3. **Environment variables** (same names as the Python server)
4. **CLI flags** (`--max-length`, `--start-index`, `--mode`, `-H`)

Unlike `websearch-cli`, the config file is **optional** — if missing, defaults
are used. This matches the server, which relies entirely on env vars.

#### Config file (TOML)

```toml
# webfetch-cli configuration
# All keys are optional — if missing, defaults are used.
# This file is copied next to the binary at build time.

default_limit = 12000            # default character limit
max_response_bytes = 5242880     # max response body bytes (5 MB)
timeout_secs = 15                # HTTP timeout in seconds
max_redirects = 8                # max redirects to follow
user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
```

#### Environment variables

| Variable | Overrides |
|----------|-----------|
| `WEBFETCH_MCP_DEFAULT_LIMIT` | config `default_limit` |
| `WEBFETCH_MCP_MAX_RESPONSE_BYTES` | config `max_response_bytes` |
| `WEBFETCH_MCP_TIMEOUT` | config `timeout_secs` |
| `WEBFETCH_MCP_MAX_REDIRECTS` | config `max_redirects` |

> **Important**: env var names match the Python server's `.env` exactly. Agents
> sharing the same environment can use either the MCP server or the CLI without
> configuration changes.

#### `.env` backward compatibility

The CLI does **not** read `.env` directly (no `python-dotenv`). It uses TOML +
env vars. The skill file instructs agents to either use `config.toml`
(recommended) or export the env vars above before running.

### Crate layout

```
cli/
  Cargo.toml
  Cargo.lock
  build.rs                  # copies config.toml next to binary
  config.toml               # gitignored — optional overrides (may be absent)
  config.toml.example       # committed template
  PLAN.md
  README.md
  src/
    main.rs                 # clap CLI + dispatch (pattern match)
    config.rs               # TOML config + env var resolution
    net.rs                  # SSRF validation (URL + resolved IPs)
    fetch.rs                # HTTP fetch, redirects, mode conversion, slicing
    output.rs               # human-readable / JSON output
```

### Dependencies

| Crate | Version | Features | Purpose |
|-------|---------|----------|---------|
| `clap` | `4` | `derive`, `env` | CLI parsing |
| `reqwest` | `0.12` | `blocking`, `json`, `rustls-tls` | HTTP (no native-tls) |
| `serde` | `1` | `derive` | Serialization |
| `serde_json` | `1` | — | JSON I/O |
| `anyhow` | `1` | — | Error handling |
| `toml` | `0.8` | — | Config parsing |
| `url` | `2` | — | URL parsing / join |
| `scraper` | `0.20` | — | HTML parse, text extraction, body extraction |
| `html2markdown` | `0.2` | — | HTML → Markdown, default `HeadingStyle::Atx` |
| `readability` | `0.3` | **default-features = false** | Readable-content extraction |

> **Crate rationale (verified against cloned sources in `/github/_rust/`):**
>
> - **`html2markdown`** is chosen over `html2md` because it defaults to
>   `HeadingStyle::Atx` (all heading levels produce `#`/`##`/…), matching the
>   server's `markdownify(heading_style="ATX")`. `html2md` renders `h1`/`h2` as
>   SETEXT (`====`/`----` underlines), which would not match.
> - `html2markdown` natively maps `script`, `style`, `noscript`, `title`, and
>   `head` to empty nodes, so the markdown/readable modes need **no manual
>   node removal**.
> - **`readability`** (`kumabook/readability`, v0.3) is the canonical Rust port
>   of `readability-lxml`; `extractor::extract(&mut reader, &url)` returns a
>   `Product { title, content, text }` where `content` is the summary HTML
>   (maps to `Document.summary()`) and `title` maps to `Document.short_title()`.
>   Its default `reqwest` feature pulls in `reqwest 0.11`, so it must be
>   disabled (`default-features = false`) to avoid a dependency conflict with
>   our `reqwest 0.12`.

### `Cargo.toml` (exact)

```toml
[package]
name = "webfetch-cli"
version = "0.1.0"
edition = "2021"
description = "CLI for fetching web pages as Markdown, text, or JSON"
license = "MIT"
build = "build.rs"

[dependencies]
clap = { version = "4", features = ["derive", "env"] }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
toml = "0.8"
url = "2"
scraper = "0.20"
html2markdown = "0.2"
readability = { version = "0.3", default-features = false }

[profile.release]
lto = true
strip = true
```

### `build.rs` (exact)

Adapted from `websearch-cli`/`fastmail-cli`. Copies `config.toml` from source
next to the compiled binary. Since the config is optional, the build proceeds
silently if the source config does not exist.

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
        // No source config; the binary will use env vars or defaults.
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
# webfetch-cli configuration
# Copy to config.toml to override defaults.
# This file is copied next to the binary at build time.
# All keys are optional — if missing, defaults are used.

default_limit = 12000            # default character limit
max_response_bytes = 5242880     # max response body bytes (5 MB)
timeout_secs = 15                # HTTP timeout in seconds
max_redirects = 8                # max redirects to follow
user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
```

## Behavior contracts

### SSRF validation (`net.rs`)

Mirror of Python `_validate_url` / `_validate_resolved_ips`. Uses the `url`
crate's parsed `Host` enum (no string-bracket stripping needed) and `std::net`
for IP classification and DNS resolution.

```rust
use anyhow::{anyhow, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use url::Host;

/// Mirror of Python ipaddress._is_blocked_ip.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || reserved_v4(v4)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || reserved_v6(v6)
        }
    }
}

/// IPv4 reserved ranges not covered by std: 0.0.0.0/8 and 240.0.0.0/4 (class E).
fn reserved_v4(v4: &Ipv4Addr) -> bool {
    let o = v4.octets();
    o[0] == 0 || o[0] >= 240
}

/// IPv6 reserved ranges not covered by std (mirrors Python ipaddress.is_reserved).
fn reserved_v6(v6: &Ipv6Addr) -> bool {
    let s = v6.segments();
    // ::ffff:0:0/96 (IPv4-mapped)
    (s[0..5].iter().all(|&x| x == 0) && s[5] == 0xffff)
    // 2001:db8::/32 (documentation)
    || (s[0] == 0x2001 && s[1] == 0x0db8)
    // 64:ff9b::/96 (NAT64 well-known prefix)
    || (s[0] == 0x0064 && s[1] == 0xff9b && s[2..5].iter().all(|&x| x == 0))
    // 100::/64 (discard)
    || (s[0] == 0x0100 && s[1..4].iter().all(|&x| x == 0))
    // 2001:10::/28 (ORCHID)
    || (s[0] == 0x2001 && (s[1] & 0xfff0) == 0x0010)
    // 2001:20::/28 (ORCHIDv2)
    || (s[0] == 0x2001 && (s[1] & 0xfff0) == 0x0020)
    // 2001:2::/48 (benchmarking)
    || (s[0] == 0x2001 && s[1] == 0x0002 && s[2] == 0)
    // 2001::/23 (reserved)
    || (s[0] == 0x2001 && (s[1] & 0xffe0) == 0)
    // 2001:1::1/128
    || (s[0] == 0x2001 && s[1] == 0x0001 && s[2] == 0 && s[3] == 0
        && s[4] == 0 && s[5] == 0 && s[6] == 0 && s[7] == 1)
    // ::1:ffff:0:0/96
    || (s[0..6].iter().all(|&x| x == 0) && s[6] == 0xffff)
    // 64:ff9b:1::/48
    || (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 1)
    // 100:64::/10
    || ((s[0] & 0xffc0) == 0x0100 && s[1] == 0x0040)
    // 5f00::/16 (SRv6)
    || (s[0] == 0x5f00)
    // 3fff::/20 (documentation)
    || ((s[0] & 0xfff0) == 0x3ff0)
}

/// Validate the URL itself: scheme, hostname presence, literal-IP check,
/// and the localhost check. Mirrors Python `_validate_url`.
pub fn validate_url(url_str: &str) -> Result<()> {
    let parsed = url::Url::parse(url_str)
        .map_err(|e| anyhow!("Invalid URL: {}", e))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(anyhow!("Blocked URL with disallowed protocol '{}'.", other)),
    }
    match parsed.host() {
        Some(Host::Ipv4(ip)) => {
            if is_blocked_ip(IpAddr::V4(ip)) {
                return Err(anyhow!("Blocked private/local address '{}'.", ip));
            }
        }
        Some(Host::Ipv6(ip)) => {
            if is_blocked_ip(IpAddr::V6(ip)) {
                return Err(anyhow!("Blocked private/local address '{}'.", ip));
            }
        }
        Some(Host::Domain(domain)) => {
            if domain.eq_ignore_ascii_case("localhost") {
                return Err(anyhow!("Blocked localhost URL."));
            }
        }
        None => return Err(anyhow!("URL must include a hostname.")),
    }
    Ok(())
}

/// Resolve a domain hostname and block it if ANY resolved IP is private/local.
/// Mirrors Python `_validate_resolved_ips` (resolution failure → proceed).
pub fn validate_resolved_ips(hostname: &str) -> Result<()> {
    match (hostname, 0).to_socket_addrs() {
        Ok(addrs) => {
            for addr in addrs {
                if is_blocked_ip(addr.ip()) {
                    return Err(anyhow!(
                        "Blocked hostname '{}' because it resolves to private/local IP '{}'.",
                        hostname,
                        addr.ip()
                    ));
                }
            }
            Ok(())
        }
        Err(_) => Ok(()), // resolution failure → let the request fail naturally
    }
}

/// Full pre-request validation: URL checks + DNS resolution check.
pub fn validate_fetch_url(url: &str) -> Result<()> {
    validate_url(url)?;
    let parsed = url::Url::parse(url)?;
    if let Some(Host::Domain(domain)) = parsed.host() {
        validate_resolved_ips(&domain)?;
    }
    Ok(())
}
```

### Fetch + redirect handling (`fetch.rs`)

Mirrors `_fetch_url`. Uses `reqwest::blocking` with `redirect::Policy::none()`
and follows redirects manually, re-validating each target.

```rust
pub struct FetchResult {
    pub url: String,
    pub final_url: String,
    pub content_type: String,
    pub title: Option<String>,
    pub body: String,
}

pub fn fetch(config: &Config, url: &str, headers: &[(String, String)]) -> Result<FetchResult> {
    let mut current_url = url.trim().to_string();
    if current_url.is_empty() {
        return Err(anyhow!("url must not be empty."));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let mut request_headers = reqwest::header::HeaderMap::new();
    request_headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_str(&config.user_agent)?,
    );
    for (name, value) in headers {
        request_headers.append(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())?,
            reqwest::header::HeaderValue::from_str(value)?,
        );
    }

    for _ in 0..=config.max_redirects {
        crate::net::validate_fetch_url(&current_url)?;
        let resp = client.get(&current_url).headers(request_headers.clone()).send()?;

        if resp.status().is_redirection() {
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    anyhow!("Redirect from {} did not include a Location header.", current_url)
                })?;
            current_url = resp
                .url()
                .join(location)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| location.to_string());
            continue;
        }

        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}: {}", resp.status(), resp.status().canonical_reason().unwrap_or("")));
        }

        let body = read_limited(&resp, config.max_response_bytes)?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let title = if content_type.contains("html") {
            extract_title(&body)
        } else {
            None
        };

        return Ok(FetchResult {
            url: url.to_string(),
            final_url: resp.url().to_string(),
            content_type,
            title,
            body,
        });
    }

    Err(anyhow!("Too many redirects; exceeded {}.", config.max_redirects))
}

/// Read the response body with a byte cap. Mirrors `_read_limited`.
fn read_limited(resp: &reqwest::blocking::Response, max_bytes: u64) -> Result<String> {
    if let Some(len) = resp.content_length() {
        if len > max_bytes {
            return Err(anyhow!("Response too large: {} bytes exceeds {}.", len, max_bytes));
        }
    }
    let mut total: u64 = 0;
    let mut bytes: Vec<u8> = Vec::new();
    for chunk in resp.chunks() {
        let chunk = chunk?;
        total += chunk.len() as u64;
        if total > max_bytes {
            return Err(anyhow!("Response too large: exceeded {} bytes while reading.", max_bytes));
        }
        bytes.extend_from_slice(&chunk);
    }
    // UTF-8 lossy decode matches Python's errors="replace" for the common case.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Extract the <title> text. Mirrors `_extract_title`.
fn extract_title(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("title").ok()?;
    let title = document
        .select(&selector)
        .next()?
        .text()
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();
    (!title.is_empty()).then_some(title)
}
```

### Mode conversion (`fetch.rs`)

```rust
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    Readable,
    Markdown,
    Txt,
    Json,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Readable => "readable",
            Mode::Markdown => "markdown",
            Mode::Txt => "txt",
            Mode::Json => "json",
        }
    }
}

/// Convert the fetched body per mode. Returns `(text, resolved_title)`.
///
/// The second element is the title to put in the response:
/// - `readable` → extracted `product.title` (fall back to `result.title`),
///   mirroring `doc.short_title() or result.title`.
/// - all other modes → `result.title` (the `<title>` tag, or `None`).
pub fn convert(result: &FetchResult, mode: Mode) -> Result<(String, Option<String>)> {
    match mode {
        Mode::Readable => {
            // readability needs a base URL for cleaning relative links.
            let base = url::Url::parse(&result.final_url)
                .or_else(|_| url::Url::parse(&result.url))
                .unwrap_or_else(|_| url::Url::parse("https://example.com").unwrap());
            let mut reader = result.body.as_bytes();
            let product = readability::extractor::extract(&mut reader, &base)?;
            let title = if product.title.is_empty() {
                result.title.clone()
            } else {
                Some(product.title)
            };
            let text = html2markdown::convert(&product.content).trim().to_string();
            Ok((text, title))
        }
        Mode::Markdown => {
            // Extract the <body> (or whole doc) and convert with ATX headings.
            // html2markdown natively skips script/style/noscript/title/head.
            let document = scraper::Html::parse_document(&result.body);
            let body_selector = scraper::Selector::parse("body").unwrap();
            let html = match document.select(&body_selector).next() {
                Some(body) => body.inner_html(),
                None => document.html(),
            };
            Ok((html2markdown::convert(&html).trim().to_string(), result.title.clone()))
        }
        Mode::Txt => {
            // Remove script/style/noscript, then normalized text.
            let mut document = scraper::Html::parse_document(&result.body);
            let selector = scraper::Selector::parse("script, style, noscript").unwrap();
            let ids: Vec<ego_tree::NodeId> = document.select(&selector).map(|e| e.id()).collect();
            for id in ids {
                if let Some(mut node) = document.tree.get_mut(id) {
                    node.detach();
                }
            }
            let text = document.root_element().text().collect::<Vec<_>>().join(" ");
            let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
            Ok((normalized, result.title.clone()))
        }
        Mode::Json => {
            let parsed: serde_json::Value = serde_json::from_str(&result.body)
                .map_err(|e| anyhow!("Failed to parse JSON: {}", e))?;
            Ok((serde_json::to_string_pretty(&parsed).unwrap_or_default(), None))
        }
    }
}

### Character-aware slicing (`fetch.rs`)

Python slices Python strings by **character**. Rust `&str` slicing uses byte
indices, which would panic on multi-byte UTF-8 boundaries. The Rust version
must slice by `char` to match `_slice_text` exactly.

```rust
pub struct Sliced {
    pub text: String,
    pub truncated: bool,
    pub next_start_index: Option<usize>,
}

/// Mirrors Python `_slice_text` (character-based, not byte-based).
pub fn slice_text(text: &str, max_length: usize, start_index: usize) -> Sliced {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let start = start_index.min(total);
    if start >= total {
        return Sliced { text: String::new(), truncated: false, next_start_index: None };
    }
    if max_length == 0 {
        return Sliced { text: chars[start..].iter().collect(), truncated: false, next_start_index: None };
    }
    let end = (start + max_length).min(total);
    let sliced: String = chars[start..end].iter().collect();
    let truncated = end < total;
    let next = if truncated { Some(end) } else { None };
    Sliced { text: sliced, truncated, next_start_index: next }
}
```

### Response building (`fetch.rs`)

Mirrors `_response` / `_error`.

```rust
pub fn build_response(
    result: &FetchResult,
    title: Option<String>,
    sliced: &Sliced,
    start_index: usize,
    mode: &str,
) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "mode": mode,
        "url": result.url,
        "final_url": result.final_url,
        "content_type": result.content_type,
        "title": title,
        "text": sliced.text,
        "truncated": sliced.truncated,
        "start_index": start_index,
        "next_start_index": sliced.next_start_index,
    })
}

pub fn error_response(message: String, mode: &str, url: &str) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "mode": mode,
        "url": url,
        "error": message,
    })
}
```

### `config.rs`

```rust
use serde::Deserialize;
use std::path::PathBuf;

pub const DEFAULT_LIMIT: usize = 12000;
pub const DEFAULT_MAX_BYTES: u64 = 5_242_880;
pub const DEFAULT_TIMEOUT: u64 = 15;
pub const DEFAULT_REDIRECTS: u32 = 8;
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ConfigFile {
    pub default_limit: usize,
    pub max_response_bytes: u64,
    pub timeout_secs: u64,
    pub max_redirects: u32,
    pub user_agent: String,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            default_limit: DEFAULT_LIMIT,
            max_response_bytes: DEFAULT_MAX_BYTES,
            timeout_secs: DEFAULT_TIMEOUT,
            max_redirects: DEFAULT_REDIRECTS,
            user_agent: DEFAULT_USER_AGENT.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub default_limit: usize,
    pub max_response_bytes: u64,
    pub timeout_secs: u64,
    pub max_redirects: u32,
    pub user_agent: String,
}

/// Resolved config: env var > config file > default. The config file is
/// optional — if missing, defaults are used.
pub fn load() -> Config {
    let file = read_config_file().unwrap_or_default();
    Config {
        default_limit: env_or_parse("WEBFETCH_MCP_DEFAULT_LIMIT").unwrap_or(file.default_limit),
        max_response_bytes: env_or_parse("WEBFETCH_MCP_MAX_RESPONSE_BYTES").unwrap_or(file.max_response_bytes),
        timeout_secs: env_or_parse("WEBFETCH_MCP_TIMEOUT").unwrap_or(file.timeout_secs),
        max_redirects: env_or_parse("WEBFETCH_MCP_MAX_REDIRECTS").unwrap_or(file.max_redirects),
        user_agent: std::env::var("WEBFETCH_MCP_USER_AGENT")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(file.user_agent),
    }
}

fn env_or_parse<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.trim().parse().ok())
}

fn resolve_config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let path = dir.join("config.toml");
    path.is_file().then_some(path)
}

fn read_config_file() -> Option<ConfigFile> {
    let path = resolve_config_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&content).ok()
}
```

### `output.rs`

```rust
use crate::fetch::{FetchResult, Sliced};

/// Human-readable output.
pub fn print_human(result: &FetchResult, title: &Option<String>, mode: &str, sliced: &Sliced, max_length: usize) {
    println!("URL: {}", result.url);
    println!("Final URL: {}", result.final_url);
    println!("Type: {}", result.content_type);
    if let Some(title) = title {
        println!("Title: {}", title);
    }
    println!("Mode: {}", mode);
    println!("---");
    println!("{}", sliced.text);
    println!("---");
    if sliced.truncated {
        if let Some(next) = sliced.next_start_index {
            println!("(truncated at {} chars, next: {})", max_length, next);
        }
    }
}

/// JSON output (matches the MCP server's tool return value exactly).
pub fn print_json(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}
```

### `main.rs`

```rust
mod config;
mod fetch;
mod net;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

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
        #[arg(long, value_enum, default_value_t = Mode::Markdown)]
        mode: Mode,

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

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Mode {
    Readable,
    Markdown,
    Txt,
    Json,
}

impl Mode {
    fn as_str(&self) -> &'static str {
        match self {
            Mode::Readable => "readable",
            Mode::Markdown => "markdown",
            Mode::Txt => "txt",
            Mode::Json => "json",
        }
    }
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
            let max_length = max_length
                .or_else(|| {
                    std::env::var("WEBFETCH_MCP_DEFAULT_LIMIT")
                        .ok()
                        .and_then(|v| v.trim().parse().ok())
                })
                .unwrap_or(config.default_limit);

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
```

## Output format

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

Matches the MCP server's tool return value exactly:

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

Error:
```json
{
  "ok": false,
  "mode": "markdown",
  "url": "https://example.com",
  "error": "HTTP 404: Not Found"
}
```

## Logging

The webfetch-mcp server does **not** log responses (unlike websearch-mcp), so
the CLI needs **no** logging module and no `chrono` dependency.

## README.md

A `cli/README.md` will be created (matching the fastmail/websearch reference
pattern):

```markdown
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
```

## .gitignore update

Create `/github/teo-mateo/ai-toolbox/mcp/webfetch-mcp/.gitignore` (none exists
yet) with:

```
# Python
__pycache__/
*.py[cod]
*.egg-info/
.venv/

# Rust CLI
cli/config.toml
cli/target/
```

## Skill file

The skill file `webfetch-cli.md` already exists at the project root
(`/github/teo-mateo/ai-toolbox/mcp/webfetch-mcp/webfetch-cli.md`). It must be
updated to match this plan:

- Use the `fetch` subcommand: `webfetch-cli fetch <URL> [--mode ...]`.
- Fix the `content_type` in output examples to `text/html` (the server stores
  the type before `;`).
- Keep the env var names identical to the server
  (`WEBFETCH_MCP_DEFAULT_LIMIT`, `WEBFETCH_MCP_MAX_RESPONSE_BYTES`,
  `WEBFETCH_MCP_TIMEOUT`, `WEBFETCH_MCP_MAX_REDIRECTS`).
- Document pagination via `--max-length` / `--start-index` / `next_start_index`.
- **The skill should always use `--json`** for agent parsing, since the JSON
  output matches the MCP server's tool return value exactly.

## Differences vs. the MCP server

| Aspect | MCP server | CLI |
|--------|-----------|-----|
| Protocol | MCP (stdio) | None (plain CLI) |
| Tools | `fetch_readable`/`fetch_markdown`/`fetch_txt`/`fetch_json` | One `fetch` subcommand with `--mode` |
| Config | env vars only | `config.toml` + env vars + CLI flags |
| Output | JSON (tool result) | Human-readable or `--json` |
| Invocation | MCP client session | Single command |
| Env var names | `WEBFETCH_MCP_*` | Same |
| Error handling | Catches exceptions → `{ok:false}` JSON | anyhow + non-zero exit + `{ok:false}` JSON |
| Timeout | 15s (httpx) | 15s (reqwest) |
| SSRF | Python ipaddress | Rust std::net + url crate (same classes) |
| Redirects | Manual, validated | Manual, validated (same) |
| Body cap | 5 MB | 5 MB (same) |
| Slicing | Python char-based | Rust char-based (same) |
| Logging | none | none |

## Implementation steps

1. **Scaffold** — Write `Cargo.toml`, `build.rs`, `config.toml.example`,
   `README.md`.
2. **`config.rs`** — Optional TOML config + env var resolution
   (`WEBFETCH_MCP_*`), defaults as fallback.
3. **`net.rs`** — SSRF validation: `is_blocked_ip`, `validate_url`,
   `validate_resolved_ips`, `validate_fetch_url`.
4. **`fetch.rs`** — HTTP fetch with manual validated redirects, limited body
   read, title extraction.
5. **`fetch.rs`** — Mode conversion (readable/markdown/txt/json) using
   `readability`, `html2markdown`, `scraper`, `serde_json`.
6. **`fetch.rs`** — Character-aware slicing + response/error builders.
7. **`output.rs`** — Human-readable + `--json` output.
8. **`main.rs`** — clap `fetch` subcommand, header parsing, dispatch.
9. **Create `.gitignore`** — Add `cli/config.toml`, `cli/target/`.
10. **Update skill file** — `webfetch-cli.md` to the `fetch` subcommand form.
11. **Build** — `cargo build --release`.
12. **Smoke-test** — Run against public pages and each mode.

## Verification

- [ ] `cargo build --release` compiles with zero warnings
- [ ] `webfetch-cli fetch https://example.com` returns Markdown
- [ ] `webfetch-cli fetch <article-url> --mode readable` extracts the article
- [ ] `webfetch-cli fetch https://example.com --mode txt` returns plain text
- [ ] `webfetch-cli fetch <json-api-url> --mode json` pretty-prints JSON
- [ ] `--json` output matches the MCP server's tool return schema
- [ ] `--max-length 0` returns the full text
- [ ] Pagination: `--start-index` + `next_start_index` round-trips correctly
- [ ] Multi-byte UTF-8 text is sliced on character boundaries (no panics)
- [ ] `localhost` and private IPs are blocked
- [ ] DNS-rebinding: a hostname resolving to `127.0.0.1` is blocked
- [ ] Redirect to a blocked address is rejected
- [ ] Missing `Location` header on redirect produces a clear error
- [ ] `-H "Accept: text/html"` custom header is sent
- [ ] Empty URL is rejected
- [ ] `--mode invalid` is rejected by clap
- [ ] `WEBFETCH_MCP_DEFAULT_LIMIT` env var overrides config/default
- [ ] Missing config file does not error (defaults used)
- [ ] Skill file `webfetch-cli.md` matches the `fetch` subcommand form
