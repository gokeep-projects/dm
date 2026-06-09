use crate::config::{self, Config};
use crate::db::Database;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const CONFIGURABLE_CHECK_IDS: &[&str] = &[
    "elasticsearch",
    "redis",
    "nginx",
    "keepalived",
    "mysql",
    "kafka",
    "java-service",
];

pub fn is_configurable_check_id(check_id: &str) -> bool {
    CONFIGURABLE_CHECK_IDS.contains(&check_id)
}

pub fn connection_config_path(config: &Config) -> PathBuf {
    config.data_dir.join("check-configs.json")
}

pub fn template_value() -> Value {
    serde_json::json!({
        "version": 1,
        "description": "DM 常规检查连接配置导入模板。保存和导入后会同步写入 ~/.dm/data/check-configs.json 与 SQLite，检查执行会读取同步后的配置。program_path 是程序可执行文件完整路径，不是所在目录；config_path/log_path 是文件路径，data_path 是数据目录。路径字段可以留空，系统会根据进程、systemd、命令行参数和配置文件自动推断。",
        "configs": {
            "elasticsearch": {
                "url": "http://127.0.0.1:9200",
                "host": "127.0.0.1",
                "port": "9200",
                "username": "",
                "password": "",
                "config_path": "",
                "data_path": "",
                "log_path": "",
                "program_path": ""
            },
            "redis": {
                "host": "127.0.0.1",
                "port": "6379",
                "password": "",
                "config_path": "",
                "data_path": "",
                "log_path": "",
                "program_path": ""
            },
            "nginx": {
                "config_path": "",
                "log_path": "",
                "program_path": ""
            },
            "keepalived": {
                "config_path": "",
                "log_path": "",
                "program_path": ""
            },
            "mysql": {
                "host": "127.0.0.1",
                "port": "3306",
                "username": "root",
                "password": "",
                "config_path": "",
                "data_path": "",
                "log_path": "",
                "program_path": ""
            },
            "kafka": {
                "host": "127.0.0.1",
                "port": "9092",
                "config_path": "",
                "data_path": "",
                "log_path": "",
                "program_path": ""
            },
            "java-service": {
                "service_prefix": "",
                "log_path": "",
                "program_path": ""
            }
        }
    })
}

pub fn read_file_configs(config: &Config) -> BTreeMap<String, Value> {
    let path = connection_config_path(config);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return BTreeMap::new();
    };
    config_map_from_value(&value)
}

pub fn config_map_from_value(value: &Value) -> BTreeMap<String, Value> {
    let source = value
        .get("configs")
        .and_then(|v| v.as_object())
        .or_else(|| value.as_object());
    let Some(source) = source else {
        return BTreeMap::new();
    };
    let mut configs = BTreeMap::new();
    for (check_id, config_value) in source {
        if check_id == "version" || check_id == "description" || check_id == "exported_at" {
            continue;
        }
        if is_configurable_check_id(check_id) && config_value.is_object() {
            configs.insert(check_id.to_string(), config_value.clone());
        }
    }
    configs
}

pub fn write_file_configs(
    config: &Config,
    configs: &BTreeMap<String, Value>,
) -> std::io::Result<PathBuf> {
    let path = connection_config_path(config);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut ordered = Map::new();
    for id in CONFIGURABLE_CHECK_IDS {
        if let Some(value) = configs.get(*id) {
            ordered.insert((*id).to_string(), value.clone());
        }
    }
    for (id, value) in configs {
        if !ordered.contains_key(id) {
            ordered.insert(id.clone(), value.clone());
        }
    }
    let payload = serde_json::json!({
        "version": 1,
        "description": "DM 检查连接配置。Web 保存、Web 导入、CLI set/import 会同步更新此文件；可直接备份或导入到其他离线环境。",
        "updated_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "configs": ordered,
    });
    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(&path, format!("{}\n", text))?;
    Ok(path)
}

pub fn export_value(config: &Config, db: &Database) -> Value {
    let mut configs = read_file_configs(config);
    for record in db.list_check_configs() {
        if is_configurable_check_id(&record.check_id) && record.value.is_object() {
            configs.insert(record.check_id, record.value);
        }
    }
    let mut ordered = Map::new();
    for id in CONFIGURABLE_CHECK_IDS {
        if let Some(value) = configs.get(*id) {
            ordered.insert((*id).to_string(), value.clone());
        }
    }
    serde_json::json!({
        "schema": "dm-check-configs/v1",
        "version": 1,
        "exported_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "config_file": connection_config_path(config).display().to_string(),
        "configs": ordered,
    })
}

pub fn sync_db_to_file(config: &Config, db: &Database) -> std::io::Result<PathBuf> {
    let mut configs = read_file_configs(config);
    for record in db.list_check_configs() {
        if is_configurable_check_id(&record.check_id) && record.value.is_object() {
            configs.insert(record.check_id, record.value);
        }
    }
    write_file_configs(config, &configs)
}

pub fn upsert_and_sync(config: &Config, db: &Database, check_id: &str, value: &Value) -> bool {
    if !is_configurable_check_id(check_id) || !value.is_object() {
        return false;
    }
    if !db.save_check_config(check_id, value) {
        return false;
    }
    let mut configs = read_file_configs(config);
    configs.insert(check_id.to_string(), value.clone());
    write_file_configs(config, &configs).is_ok()
}

pub fn import_configs_and_sync(
    config: &Config,
    db: &Database,
    imported: Vec<(String, Value)>,
) -> Vec<String> {
    let mut configs = read_file_configs(config);
    let mut saved = Vec::new();
    for (check_id, value) in imported {
        if !is_configurable_check_id(&check_id) || !value.is_object() {
            continue;
        }
        if db.save_check_config(&check_id, &value) {
            configs.insert(check_id.clone(), value);
            saved.push(check_id);
        }
    }
    let _ = write_file_configs(config, &configs);
    saved
}

pub fn load_config_value(config: &Config, db: &Database, check_id: &str) -> Option<Value> {
    let file_value = read_file_configs(config).remove(check_id);
    let db_value = db.get_check_config(check_id).map(|record| record.value);
    match (file_value, db_value) {
        (Some(mut file), Some(db_value)) => {
            merge_object_values(&mut file, &db_value);
            Some(file)
        }
        (Some(file), None) => {
            let _ = db.save_check_config(check_id, &file);
            Some(file)
        }
        (None, Some(db_value)) => Some(db_value),
        (None, None) => None,
    }
}

fn merge_object_values(base: &mut Value, overlay: &Value) {
    let (Some(base_obj), Some(overlay_obj)) = (base.as_object_mut(), overlay.as_object()) else {
        return;
    };
    for (key, value) in overlay_obj {
        base_obj.insert(key.clone(), value.clone());
    }
}

pub fn open_runtime_db(config: &Config) -> Database {
    config::ensure_user_dirs(config);
    Database::open(&config::db_path(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(name: &str) -> Config {
        let base = std::env::temp_dir().join(format!(
            "dm-check-config-store-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        Config {
            scripts_dir: base.join("scripts"),
            user_scripts_dir: base.join("user-scripts"),
            port: 3399,
            bind: "127.0.0.1".to_string(),
            theme: "dark".to_string(),
            language: "zh".to_string(),
            log_dir: base.join("logs"),
            data_dir: base.join("data"),
        }
    }

    #[test]
    fn upsert_writes_database_and_connection_config_file() {
        let cfg = temp_config("upsert");
        let db = open_runtime_db(&cfg);
        let value = serde_json::json!({"host":"10.0.0.12","port":"9092"});
        assert!(upsert_and_sync(&cfg, &db, "kafka", &value));
        assert_eq!(
            db.get_check_config("kafka").unwrap().value["host"],
            "10.0.0.12"
        );
        let file_configs = read_file_configs(&cfg);
        assert_eq!(file_configs["kafka"]["port"], "9092");
    }

    #[test]
    fn export_merges_file_and_database_configs() {
        let cfg = temp_config("export");
        let db = open_runtime_db(&cfg);
        let mut file_configs = BTreeMap::new();
        file_configs.insert("redis".to_string(), serde_json::json!({"host":"10.0.0.10"}));
        write_file_configs(&cfg, &file_configs).unwrap();
        assert!(db.save_check_config("mysql", &serde_json::json!({"host":"10.0.0.11"})));
        let exported = export_value(&cfg, &db);
        assert_eq!(exported["configs"]["redis"]["host"], "10.0.0.10");
        assert_eq!(exported["configs"]["mysql"]["host"], "10.0.0.11");
    }
}
