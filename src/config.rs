use std::{fs, path::Path};

use serde::Deserialize;
use anyhow::Result;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_model")]
    pub model: String,
    
    #[serde(default = "default_toxicity")]
    pub toxicity: String, // "Low", "Medium", "High"

    #[serde(default)]
    pub ignore: Vec<String>,
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_toxicity() -> String {
    "Medium".to_string()
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Path::new(".git-sensei.toml");

        if config_path.exists() {
            let content = fs::read_to_string(config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config {
                model: default_model(),
                toxicity: default_toxicity(),
                ignore: vec!["*.lock".to_string(), "target/*".to_string()],
            })
        }
    }
}
