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
pub(crate) struct SerpapiHit {
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
    pub(crate) results: Option<Vec<ExaHit>>,
}

#[derive(Deserialize, serde::Serialize)]
pub(crate) struct ExaHit {
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
