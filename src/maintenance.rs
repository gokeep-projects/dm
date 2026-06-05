use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub operator: String,
    pub timestamp: String,
    pub status: String,
    pub result: String,
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".dm").join("data")
}

fn record_path() -> PathBuf {
    data_dir().join("maintenance_record")
}

pub fn ensure_data_dir() {
    let _ = std::fs::create_dir_all(data_dir());
}

#[allow(dead_code)]
fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

pub fn list_records(category: Option<&str>) -> Vec<MaintenanceRecord> {
    let path = record_path();
    if !path.exists() {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut records: Vec<MaintenanceRecord> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if let Some(cat) = category {
        records.retain(|r| r.category == cat);
    }
    records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    records
}

pub fn get_record(id: &str) -> Option<MaintenanceRecord> {
    list_records(None).into_iter().find(|r| r.id == id)
}

pub fn create_record(
    title: &str,
    description: &str,
    category: &str,
    operator: &str,
) -> Result<MaintenanceRecord, String> {
    ensure_data_dir();
    let id = format!("{}-{}", Local::now().format("%Y%m%d%H%M%S"), rand_suffix());
    let record = MaintenanceRecord {
        id,
        title: title.to_string(),
        description: description.to_string(),
        category: category.to_string(),
        operator: operator.to_string(),
        timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        status: "open".to_string(),
        result: String::new(),
    };
    append_record(&record)?;
    Ok(record)
}

pub fn update_record(
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    category: Option<&str>,
    operator: Option<&str>,
    result: Option<&str>,
    status: Option<&str>,
) -> Result<MaintenanceRecord, String> {
    let path = record_path();
    if !path.exists() {
        return Err("记录文件不存在".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut records: Vec<MaintenanceRecord> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let mut updated = None;
    for r in records.iter_mut() {
        if r.id == id {
            if let Some(v) = title {
                r.title = v.to_string();
            }
            if let Some(v) = description {
                r.description = v.to_string();
            }
            if let Some(v) = category {
                r.category = v.to_string();
            }
            if let Some(v) = operator {
                r.operator = v.to_string();
            }
            if let Some(v) = result {
                r.result = v.to_string();
            }
            if let Some(v) = status {
                r.status = if v == "completed" {
                    "completed"
                } else {
                    "open"
                }
                .to_string();
            }
            updated = Some(r.clone());
            break;
        }
    }

    let Some(record) = updated else {
        return Err("记录不存在".to_string());
    };
    write_records(&records)?;
    Ok(record)
}

pub fn complete_record(id: &str, result: &str) -> Result<(), String> {
    let path = record_path();
    if !path.exists() {
        return Err("记录文件不存在".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut records: Vec<MaintenanceRecord> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let mut found = false;
    for r in records.iter_mut() {
        if r.id == id {
            if r.status == "completed" {
                r.status = "open".to_string();
            } else {
                r.status = "completed".to_string();
                r.result = result.to_string();
            }
            found = true;
            break;
        }
    }

    if !found {
        return Err("记录不存在".to_string());
    }

    write_records(&records)?;
    Ok(())
}

pub fn delete_record(id: &str) -> Result<(), String> {
    let path = record_path();
    if !path.exists() {
        return Err("记录文件不存在".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let records: Vec<MaintenanceRecord> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let new_records: Vec<&MaintenanceRecord> = records.iter().filter(|r| r.id != id).collect();
    if new_records.len() == records.len() {
        return Err("记录不存在".to_string());
    }

    let new_content: String = new_records
        .iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, new_content).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_records(records: &[MaintenanceRecord]) -> Result<(), String> {
    let path = record_path();
    let new_content: String = records
        .iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, new_content).map_err(|e| e.to_string())
}

fn append_record(record: &MaintenanceRecord) -> Result<(), String> {
    let path = record_path();
    let json = serde_json::to_string(record).map_err(|e| e.to_string())?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{}", json).map_err(|e| e.to_string())?;
    Ok(())
}

fn rand_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:04}", t.subsec_millis() % 10000)
}
