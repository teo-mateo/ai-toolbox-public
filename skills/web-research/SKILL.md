---
name: web-research
description: Perform multi-step web research by combining the websearch-cli Rust binary (to find relevant pages) and the webfetch-cli Rust binary (to read their content). Use this when a research task needs more than a single search — discovering sources, then fetching and reading the actual pages. Use web-research instead of websearch-cli or webfetch-cli alone when the task requires both discovery AND deep reading.
---

# Web Research

Orchestrate the `websearch-cli` and `webfetch-cli` Rust binaries together to
run a full research workflow: **discover** relevant sources with a search,
then **read** the actual page content with a fetch. Both CLIs talk directly
to their targets — no MCP server, no HTTP server, no Python venv required.

## Binaries

| Tool | Binary | Purpose |
|------|--------|---------|
| `websearch-cli` | `/github/teo-mateo/ai-toolbox/mcp/websearch-mcp/cli/target/release/websearch-cli` | Search the web (SerpAPI/Exa) |
| `webfetch-cli` | `/github/teo-mateo/ai-toolbox/mcp/webfetch-mcp/cli/target/release/webfetch-cli` | Fetch & read a page (Markdown/text/JSON) |

If either binary does not exist, build it first:

```bash
cd /github/teo-mateo/ai-toolbox/mcp/websearch-mcp/cli && cargo build --release
cd /github/teo-mateo/ai-toolbox/mcp/webfetch-mcp/cli && cargo build --release
```

## Research workflow

Run these steps in order. Always use `--json` on both CLIs so the output is
machine-parseable and matches the MCP server schemas.

### 1. Discover sources (search)

```bash
websearch-cli search "<query>" --limit 10 --json
```

- Use natural-language queries. Run several searches with different
  phrasings to cover the topic.
- Prefer results from authoritative domains (`.gov`, `.edu`, official docs,
  primary sources).
- Collect the `url` fields from the JSON `results` array.

### 2. Read the sources (fetch)

For each promising URL, fetch its content. Pick the right `--mode`:

| Mode | When to use |
|------|-------------|
| `readable` | News articles / blog posts — extract the main article body |
| `markdown` | General HTML pages — full body as Markdown (default) |
| `txt` | When you only need clean plain text |
| `json` | JSON APIs / structured endpoints |

```bash
webfetch-cli fetch "<url>" --mode readable --json
webfetch-cli fetch "<url>" --mode markdown --json
```

### 3. Paginate long pages

If a fetch returns `"truncated": true`, continue reading using
`next_start_index`:

```bash
webfetch-cli fetch "<url>" --mode readable --max-length 12000 --start-index <next_start_index> --json
```

Repeat until `"truncated": false`.

### 4. Synthesize

Combine the fetched content across sources into a coherent answer, citing
each source's `url` / `final_url`. Cross-check facts across multiple
independent sources before treating them as established.

## Practical tips

- **Search first, fetch second.** Search snippets are often enough to
  disqualify a source; only fetch the pages that look relevant.
- **Batch queries.** Run 2–3 search queries per topic to reduce bias from a
  single engine/ranking.
- **Read the primary source.** Prefer fetching the original document over
  secondary summaries when the search returns both.
- **Use `readable` for articles** — it strips nav/ads and returns the main
  body, which is cleaner for note-taking than full-page `markdown`.
- **`--max-length 0`** returns the entire page in one shot (no pagination).
- **JSON APIs** (e.g. GitHub, REST endpoints) are best read with `--mode json`.

## Example end-to-end

```bash
# 1. Discover
websearch-cli search "Rust async runtime comparison 2025" --limit 10 --json

# 2. Fetch the top result's article body
webfetch-cli fetch "https://tokio.rs/blog/..." --mode readable --json

# 3. Fetch a second source for cross-checking
webfetch-cli fetch "https://blog.rust-lang.org/..." --mode readable --json

# 4. Synthesize the findings into an answer with citations
```

## Security

Both CLIs enforce the same SSRF protections as their MCP counterparts:

- Only `http://` and `https://` schemes allowed
- `localhost`, loopback, private, link-local, multicast, reserved, and
  unspecified IP addresses are blocked
- Hostnames are resolved before each request (DNS-rebinding safe)
- Each redirect target is re-validated before following
- Maximum response size (default 5 MB) and 8 redirects

## When to use this skill

- Any task that needs **both** finding sources **and** reading their content
  (e.g. "summarize what the community says about X", "compare approaches to
  Y", "research the latest state of Z").
- When you want a dependency-free research pipeline without starting the MCP
  servers or managing Python venvs.
- When the MCP servers are unavailable.

## When NOT to use this skill

- For a single quick search with no need to read pages — use `websearch-cli`
  alone.
- For fetching a known URL with no discovery step — use `webfetch-cli` alone.
- When you need JavaScript-rendered pages — use `browser-fetch-mcp`
  (Playwright) instead.
- When you need the MCP protocol / tool names (`web_search`,
  `fetch_*`) for a tool-calling workflow.
