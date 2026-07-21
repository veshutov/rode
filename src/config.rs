use anyhow::{Context, Result};
use serde::Deserialize;

use crate::agent::provider::LLMProviderConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub model: String,
    pub provider: LLMProviderConfig,
}

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .build()
            .with_context(|| format!("Failed to read config file: {}", path))?;

        settings
            .try_deserialize::<AppConfig>()
            .with_context(|| format!("Failed to parse config file: {}", path))
    }
}
