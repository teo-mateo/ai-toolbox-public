use crate::config::Config;
use anyhow::{anyhow, Result};

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
        let mut resp = client.get(&current_url).headers(request_headers.clone()).send()?;

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

        let body = read_limited(&mut resp, config.max_response_bytes)?;
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
fn read_limited(resp: &mut reqwest::blocking::Response, max_bytes: u64) -> Result<String> {
    use std::io::Read;
    if let Some(len) = resp.content_length() {
        if len > max_bytes {
            return Err(anyhow!("Response too large: {} bytes exceeds {}.", len, max_bytes));
        }
    }
    let mut total: u64 = 0;
    let mut bytes: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > max_bytes {
            return Err(anyhow!("Response too large: exceeded {} bytes while reading.", max_bytes));
        }
        bytes.extend_from_slice(&buf[..n]);
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

#[derive(Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
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
