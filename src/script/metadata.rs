use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptParam {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub default: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub feature: String,
    #[serde(default)]
    pub example: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub params: Vec<ScriptParam>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}
fn default_category() -> String {
    "未分类".to_string()
}

impl ScriptMetadata {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
