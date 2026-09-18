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
