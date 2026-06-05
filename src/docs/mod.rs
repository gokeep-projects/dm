use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMeta {
    pub id: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doc {
    pub meta: DocMeta,
    pub content: String,
}

fn docs_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".dm").join("docs")
}

pub fn ensure_docs_dir() {
    let _ = std::fs::create_dir_all(docs_dir());
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

pub fn list_docs(category: Option<&str>) -> Vec<DocMeta> {
    let dir = docs_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut docs = Vec::new();
    for entry in std::fs::read_dir(&dir).into_iter().flatten() {
        let Ok(e) = entry else {
            continue;
        };
        let p = e.path();
        if p.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(doc) = load_doc_meta(&p) {
            if let Some(cat) = category {
                if doc.category != cat {
                    continue;
                }
            }
            docs.push(doc);
        }
    }
    docs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    docs
}

fn load_doc_meta(path: &std::path::Path) -> Option<DocMeta> {
    let content = std::fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    let meta = std::fs::metadata(path).ok()?;
    let mut title = id.clone();
    let mut category = "未分类".to_string();
    let mut tags = Vec::new();
    let mut created_at = String::new();
    let mut updated_at = String::new();

    for line in content.lines() {
        if line.starts_with("# ") && title == id {
            title = line[2..].trim().to_string();
        }
        if line.starts_with("<!-- category:") {
            category = line
                .trim_start_matches("<!-- category:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
        }
        if line.starts_with("<!-- tags:") {
            let t = line
                .trim_start_matches("<!-- tags:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
            tags = t
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if line.starts_with("<!-- created:") {
            created_at = line
                .trim_start_matches("<!-- created:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
        }
        if line.starts_with("<!-- updated:") {
            updated_at = line
                .trim_start_matches("<!-- updated:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
        }
    }

    if created_at.is_empty() {
        created_at = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|_d| Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
    }
    if updated_at.is_empty() {
        updated_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|_d| Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
    }

    Some(DocMeta {
        id,
        title,
        category,
        tags,
        created_at,
        updated_at,
        size_bytes: meta.len(),
    })
}

pub fn get_doc(id: &str) -> Option<Doc> {
    let safe_id = sanitize_id(id);
    let path = docs_dir().join(format!("{}.md", safe_id));
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let meta = load_doc_meta(&path)?;
    Some(Doc { meta, content })
}

pub fn create_doc(id: &str, title: &str, category: &str, content: &str) -> Result<DocMeta, String> {
    let safe_id = sanitize_id(id);
    if safe_id.is_empty() {
        return Err("无效的文档 ID".to_string());
    }
    ensure_docs_dir();
    let path = docs_dir().join(format!("{}.md", safe_id));
    if path.exists() {
        return Err("文档已存在".to_string());
    }

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let full_content = format!("<!-- category:{} -->\n<!-- tags: -->\n<!-- created:{} -->\n<!-- updated:{} -->\n\n# {}\n\n{}", category, now, now, title, content);
    std::fs::write(&path, &full_content).map_err(|e| e.to_string())?;
    load_doc_meta(&path).ok_or_else(|| "创建失败".to_string())
}

#[allow(dead_code)]
pub fn update_doc(
    id: &str,
    title: Option<&str>,
    category: Option<&str>,
    tags: Option<&str>,
    content: Option<&str>,
) -> Result<DocMeta, String> {
    let safe_id = sanitize_id(id);
    let path = docs_dir().join(format!("{}.md", safe_id));
    if !path.exists() {
        return Err("文档不存在".to_string());
    }

    let existing = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let _new_title = title.map(|s| s.to_string());
    let _new_category = category.map(|s| s.to_string());
    let _new_tags = tags.map(|s| s.to_string());
    let _new_content = content.map(|s| s.to_string());

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();

    if let Some(t) = title {
        for line in lines.iter_mut() {
            if line.starts_with("# ") {
                *line = format!("# {}", t);
                break;
            }
        }
    }

    if let Some(c) = category {
        for line in lines.iter_mut() {
            if line.starts_with("<!-- category:") {
                *line = format!("<!-- category:{} -->", c);
                break;
            }
        }
    }

    if let Some(t) = tags {
        for line in lines.iter_mut() {
            if line.starts_with("<!-- tags:") {
                *line = format!("<!-- tags:{} -->", t);
                break;
            }
        }
    }

    for line in lines.iter_mut() {
        if line.starts_with("<!-- updated:") {
            *line = format!("<!-- updated:{} -->", now);
            break;
        }
    }

    if let Some(c) = content {
        let mut result = Vec::new();
        let mut past_header = false;
        for line in lines {
            if !past_header && (line.starts_with("# ") || line.trim().is_empty()) {
                past_header = true;
                result.push(line);
                result.push(String::new());
                result.extend(c.lines().map(|l| l.to_string()));
                continue;
            }
            if past_header && !line.starts_with("<!-- ") {
                continue;
            }
            result.push(line);
        }
        lines = result;
    }

    let final_content = lines.join("\n");
    std::fs::write(&path, &final_content).map_err(|e| e.to_string())?;
    load_doc_meta(&path).ok_or_else(|| "更新失败".to_string())
}

pub fn delete_doc(id: &str) -> Result<(), String> {
    let safe_id = sanitize_id(id);
    let path = docs_dir().join(format!("{}.md", safe_id));
    if !path.exists() {
        return Err("文档不存在".to_string());
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}

pub fn upload_file(doc_id: &str, filename: &str, data: &[u8]) -> Result<String, String> {
    let safe_id = sanitize_id(doc_id);
    if safe_id.is_empty() {
        return Err("无效的文档 ID".to_string());
    }
    ensure_docs_dir();
    let attach_dir = docs_dir().join(format!("{}_attachments", safe_id));
    std::fs::create_dir_all(&attach_dir).map_err(|e| e.to_string())?;
    let safe_name: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    if safe_name.is_empty() {
        return Err("无效的文件名".to_string());
    }
    let path = attach_dir.join(&safe_name);
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(format!("{}/{}", safe_id, safe_name))
}

pub fn download_file(doc_id: &str, filename: &str) -> Result<Vec<u8>, String> {
    let safe_id = sanitize_id(doc_id);
    let safe_name: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    let path = docs_dir()
        .join(format!("{}_attachments", safe_id))
        .join(&safe_name);
    if !path.exists() {
        return Err("文件不存在".to_string());
    }
    std::fs::read(&path).map_err(|e| e.to_string())
}

pub fn list_attachments(doc_id: &str) -> Vec<String> {
    let safe_id = sanitize_id(doc_id);
    let attach_dir = docs_dir().join(format!("{}_attachments", safe_id));
    if !attach_dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&attach_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

pub fn list_dir_files(dir_path: &str) -> Result<Vec<serde_json::Value>, String> {
    let path = std::path::PathBuf::from(dir_path);
    if !path.exists() {
        return Err("目录不存在".to_string());
    }
    if !path.is_dir() {
        return Err("不是目录".to_string());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
        let Ok(e) = entry else { continue };
        let p = e.path();
        let meta = e.metadata().map_err(|e| e.to_string())?;
        files.push(serde_json::json!({
            "name": p.file_name().unwrap_or_default().to_string_lossy(),
            "path": p.display().to_string(),
            "is_dir": p.is_dir(),
            "size": meta.len(),
            "modified": meta.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }));
    }
    files.sort_by(|a, b| {
        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .cmp(b["name"].as_str().unwrap_or(""))
        })
    });
    Ok(files)
}
