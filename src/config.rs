use serde::Deserialize;
use std::fs;

use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct EditorConfig {
    pub theme: String,
    pub tab_width: usize,
    pub syntax_map: HashMap<String, String>,
    pub vcur: Option<String>,
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
