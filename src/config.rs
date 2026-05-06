use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub auth: AuthConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub api_token: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        if let Ok(token) = std::env::var("MONDAY_API_TOKEN") {
            if !token.is_empty() {
                return Ok(Config {
                    auth: AuthConfig { api_token: token },
                });
            }
        }
        let path = Self::config_path()?;
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("read config {}", path.display()))?;
            let cfg: Config = toml::from_str(&raw).context("parse config toml")?;
            return Ok(cfg);
        }
        Ok(Config::default())
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("create config dir")?;
        }
        let raw = toml::to_string_pretty(self).context("serialize config")?;
        fs::write(&path, raw).with_context(|| format!("write config {}", path.display()))?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("no config dir")?;
        Ok(base.join("monday-tui").join("config.toml"))
    }

    pub fn has_token(&self) -> bool {
        !self.auth.api_token.trim().is_empty()
    }
}
