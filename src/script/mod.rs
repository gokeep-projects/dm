pub mod executor;
pub mod metadata;

use self::metadata::ScriptMetadata;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub id: String,
    pub name: String,
    pub description: String,
    pub feature: String,
    pub example: String,
    pub path: PathBuf,
    pub metadata: Option<ScriptMetadata>,
    pub modified: Option<String>,
    pub category: String,
    #[serde(default)]
    pub user_managed: bool,
}

/// 从多个目录发现脚本（去重：先出现的目录优先）
pub fn discover_scripts(dirs: &[PathBuf]) -> Result<Vec<Script>> {
    let mut seen = std::collections::HashSet::new();
    let mut scripts = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if seen.contains(&dir_name) {
                continue;
            }
            seen.insert(dir_name.clone());

            let script_file = find_script_file(&path, &dir_name);
            if let Some(script_path) = script_file {
                let metadata_path = path.join(".dm.toml");
                let metadata = if metadata_path.exists() {
                    ScriptMetadata::load(&metadata_path).ok()
                } else {
                    None
                };

                let description = metadata
                    .as_ref()
                    .map(|m| m.description.clone())
                    .unwrap_or_else(|| "无描述".to_string());
                let name = metadata
                    .as_ref()
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| dir_name.clone());
                let category = metadata
                    .as_ref()
                    .map(|m| m.category.clone())
                    .unwrap_or_else(|| "未分类".to_string());
                let feature = metadata
                    .as_ref()
                    .map(|m| m.feature.clone())
                    .unwrap_or_default();
                let example = metadata
                    .as_ref()
                    .map(|m| m.example.clone())
                    .unwrap_or_default();
                let modified = std::fs::metadata(&script_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Local> = t.into();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    });
                scripts.push(Script {
                    id: dir_name,
                    name,
                    description,
                    feature,
                    example,
                    path: script_path,
                    metadata,
                    modified,
                    category,
                    user_managed: false,
                });
            }
        }
    }
    scripts.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(scripts)
}

/// 从多个目录查找指定 ID 的脚本
pub fn find_script(dirs: &[PathBuf], id: &str) -> Result<Option<Script>> {
    let scripts = discover_scripts(dirs)?;
    Ok(scripts.into_iter().find(|s| s.id == id))
}

fn find_script_file(dir: &Path, dir_name: &str) -> Option<PathBuf> {
    let sh_file = dir.join(format!("{}.sh", dir_name));
    if sh_file.exists() {
        return Some(sh_file);
    }
    if let Some(p) = find_by_extension(dir) {
        return Some(p);
    }
    let same_name = dir.join(dir_name);
    if same_name.is_file() {
        return Some(same_name);
    }
    None
}

fn find_by_extension(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str())?.to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "sh" | "bash"
                | "zsh"
                | "ksh"
                | "py"
                | "python"
                | "js"
                | "mjs"
                | "pl"
                | "perl"
                | "rb"
                | "lua"
                | "php"
                | "awk"
                | "expect"
                | "exp"
                | "run"
                | "bin"
        ) {
            return Some(p);
        }
    }
    None
}
