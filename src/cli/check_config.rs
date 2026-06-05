use crate::cli::util;
use crate::config;
use crate::db::Database;
use anyhow::{anyhow, Result};
use colored::*;
use serde_json::Value;
use tabled::{builder::Builder, settings::Style};

pub fn get(check_id: &str, json: bool) -> Result<()> {
    let db = open_db();
    let record = db.get_check_config(check_id);
    let value = record
        .as_ref()
        .map(|r| r.value.clone())
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
    let db = open_db();
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

    if !db.save_check_config(check_id, &current) {
        return Err(anyhow!("保存检查配置失败"));
    }
    println!(
        "  {} {}",
        util::status_label("ok"),
        format!("{} 配置已保存", check_id).bright_white()
    );
    get(check_id, false)
}

fn open_db() -> Database {
    let cfg = config::Config::load();
    config::ensure_user_dirs(&cfg);
    Database::open(&crate::config::db_path(&cfg))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
