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

    toml::from_str::<ConfigFile>(&content)
        .with_context(|| format!("Failed to parse config file {}", path.display()))
}
