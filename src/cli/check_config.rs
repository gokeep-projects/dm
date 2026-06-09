use crate::cli::util;
use crate::config;
use anyhow::{anyhow, Result};
use colored::*;
use serde_json::Value;
use std::path::Path;
use tabled::{builder::Builder, settings::Style};

const CONFIGURABLE_CHECK_IDS: &[&str] = &[
    "elasticsearch",
    "redis",
    "nginx",
    "keepalived",
    "mysql",
    "kafka",
    "java-service",
];

pub fn get(check_id: &str, json: bool) -> Result<()> {
    let cfg = config::Config::load();
    let db = crate::check_config_store::open_runtime_db(&cfg);
    let record = db.get_check_config(check_id);
    let value = record
        .as_ref()
        .map(|r| r.value.clone())
        .or_else(|| crate::check_config_store::load_config_value(&cfg, &db, check_id))
        .unwrap_or_else(|| serde_json::json!({}));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "check_id": check_id,
                "value": value,
                "updated_at": record.map(|r| r.updated_at).unwrap_or_default(),
            }))?
        );
        return Ok(());
    }

    util::print_heading("检查配置", Some(check_id));
    if value.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!(
            "  {} 当前未配置，可使用 {}",
            util::status_label("warn"),
            format!("dm check-config set {} host=127.0.0.1 port=9200", check_id).bright_cyan()
        );
        return Ok(());
    }

    let mut builder = Builder::default();
    builder.push_record(["配置项", "值"]);
    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let rendered = if key == "password" && !val.as_str().unwrap_or("").is_empty() {
                "********".to_string()
            } else {
                value_to_string(val)
            };
            builder.push_record([key.to_string(), rendered]);
        }
    }
    let mut table = builder.build();
    table.with(Style::ascii_rounded());
    for line in table.to_string().lines() {
        println!("  {}", line.white());
    }
    if let Some(record) = record {
        println!();
        println!(
            "  {} 更新时间: {}",
            util::status_label("info"),
            record.updated_at
        );
    }
    Ok(())
}

pub fn set(check_id: &str, values: &[String]) -> Result<()> {
    if values.is_empty() {
        return Err(anyhow!("至少需要一个 KEY=VALUE 配置项"));
    }
    let cfg = config::Config::load();
    let db = crate::check_config_store::open_runtime_db(&cfg);
    let mut current = db
        .get_check_config(check_id)
        .map(|r| r.value)
        .unwrap_or_else(|| serde_json::json!({}));
    if !current.is_object() {
        current = serde_json::json!({});
    }
    let obj = current
        .as_object_mut()
        .ok_or_else(|| anyhow!("检查配置不是 JSON 对象"))?;

    for item in values {
        let Some((key, value)) = item.split_once('=') else {
            return Err(anyhow!("配置项格式错误: {}，应为 KEY=VALUE", item));
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(anyhow!("配置项名称非法: {}", key));
        }
        obj.insert(key.to_string(), serde_json::json!(value.trim()));
    }

    if !crate::check_config_store::upsert_and_sync(&cfg, &db, check_id, &current) {
        return Err(anyhow!("保存检查配置失败"));
    }
    println!(
        "  {} {}",
        util::status_label("ok"),
        format!(
            "{} 配置已保存，并同步到 {}",
            check_id,
            crate::check_config_store::connection_config_path(&cfg).display()
        )
        .bright_white()
    );
    get(check_id, false)
}

pub fn template(output: Option<std::path::PathBuf>) -> Result<()> {
    let text = serde_json::to_string_pretty(&template_value())?;
    if let Some(path) = output {
        std::fs::write(&path, format!("{}\n", text))?;
        println!(
            "  {} {}",
            util::status_label("ok"),
            format!("模板已写入 {}", path.display()).bright_white()
        );
    } else {
        println!("{}", text);
    }
    Ok(())
}

pub fn import(file: &Path) -> Result<()> {
    let raw = std::fs::read_to_string(file)?;
    let payload: Value = serde_json::from_str(&raw).map_err(|e| anyhow!("JSON 解析失败: {}", e))?;
    let (imported, skipped, errors) = normalize_import_payload(&payload);
    if imported.is_empty() {
        return Err(anyhow!(
            "未导入任何配置{}",
            if errors.is_empty() {
                String::new()
            } else {
                format!(": {}", errors.join("; "))
            }
        ));
    }

    let cfg = config::Config::load();
    let db = crate::check_config_store::open_runtime_db(&cfg);
    let mut to_save = Vec::new();
    for (check_id, mut value) in imported {
        if value
            .get("password")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().is_empty())
            .unwrap_or(false)
        {
            if let Some(existing) = db.get_check_config(&check_id) {
                if let Some(password) = existing.value.get("password").and_then(|v| v.as_str()) {
                    if !password.is_empty() {
                        if let Some(obj) = value.as_object_mut() {
                            obj.insert("password".to_string(), serde_json::json!(password));
                        }
                    }
                }
            }
        }
        to_save.push((check_id, value));
    }
    let saved = crate::check_config_store::import_configs_and_sync(&cfg, &db, to_save);

    util::print_heading("连接配置导入", Some(&file.display().to_string()));
    println!(
        "  {} 已导入 {} 个检查配置: {}",
        util::status_label("ok"),
        saved.len(),
        saved.join(", ").bright_cyan()
    );
    println!(
        "  {} 配置文件已同步: {}",
        util::status_label("info"),
        crate::check_config_store::connection_config_path(&cfg)
            .display()
            .to_string()
            .bright_cyan()
    );
    if !skipped.is_empty() {
        println!(
            "  {} 已跳过未知检查: {}",
            util::status_label("warn"),
            skipped.join(", ").yellow()
        );
    }
    if !errors.is_empty() {
        println!(
            "  {} 部分配置未导入: {}",
            util::status_label("warn"),
            errors.join("; ").yellow()
        );
    }
    Ok(())
}

pub fn export(output: Option<std::path::PathBuf>) -> Result<()> {
    let cfg = config::Config::load();
    let db = crate::check_config_store::open_runtime_db(&cfg);
    let value = crate::check_config_store::export_value(&cfg, &db);
    let _ = crate::check_config_store::sync_db_to_file(&cfg, &db);
    let text = serde_json::to_string_pretty(&value)?;
    if let Some(path) = output {
        std::fs::write(&path, format!("{}\n", text))?;
        println!(
            "  {} {}",
            util::status_label("ok"),
            format!("连接配置已导出到 {}", path.display()).bright_white()
        );
    } else {
        println!("{}", text);
    }
    Ok(())
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn template_value() -> Value {
    crate::check_config_store::template_value()
}

fn normalize_import_payload(payload: &Value) -> (Vec<(String, Value)>, Vec<String>, Vec<String>) {
    let source = payload
        .get("configs")
        .and_then(|v| v.as_object())
        .or_else(|| payload.as_object());

    let Some(source) = source else {
        return (
            Vec::new(),
            Vec::new(),
            vec!["导入文件必须是 JSON 对象，或包含 configs 对象".to_string()],
        );
    };

    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();
    for (check_id, value) in source {
        if check_id == "version" || check_id == "description" {
            continue;
        }
        if !CONFIGURABLE_CHECK_IDS.contains(&check_id.as_str()) {
            skipped.push(check_id.to_string());
            continue;
        }
        if !value.is_object() {
            errors.push(format!("{} 配置必须是 JSON 对象", check_id));
            continue;
        }
        imported.push((check_id.to_string(), value.clone()));
    }
    (imported, skipped, errors)
}
