use serde::Deserialize;
use std::fs;

use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AiConfig {
    pub default_model: Option<String>,
    pub max_tokens_default: Option<usize>,
    pub temperature_default: Option<f32>,
    pub timeout_ms_default: Option<u64>,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Deserialize)]
pub struct EditorConfig {
    pub theme: String,
    pub tab_width: usize,
    pub syntax_map: HashMap<String, String>,
    pub vcur: Option<String>,
    pub ai: Option<AiConfig>,
}

impl EditorConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let home = home::home_dir().ok_or("Failed to get home directory")?;
        let path = home.join(".vedit.toml");
        let content = fs::read_to_string(path)?;
        let config: EditorConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
