use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub struct Database {
    path: std::path::PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecRecord {
    pub id: i64,
    pub script_id: String,
    pub script_name: String,
    pub timestamp: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub output_lines: usize,
    pub params: Value,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct DailyStats {
    pub date: String,
    pub total: usize,
    pub success: usize,
    pub failure: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRecord {
    pub id: String,
    pub alert_type: String,
    pub level: String,
    pub title: String,
    pub message: String,
    pub service_name: Option<String>,
    pub pid: Option<String>,
    pub log_path: Option<String>,
    pub summary: Option<String>,
    pub handling: Option<String>,
    pub evidence_json: String,
    pub suggestions_json: String,
    pub commands_json: String,
    pub first_seen: String,
    pub last_seen: String,
    pub occurrence_count: usize,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConfigRecord {
    pub check_id: String,
    pub value: serde_json::Value,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricPoint {
    pub timestamp: String,
    pub ts_ms: i64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub load_one: f64,
    pub load_ratio: f64,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleOverrideRecord {
    pub rule_id: String,
    pub value: serde_json::Value,
    pub updated_at: String,
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
        }
    }
}

impl Database {
    fn conn(&self) -> SqlResult<Connection> {
        Connection::open(&self.path)
    }

    pub fn open(path: &Path) -> Self {
        let db = Self {
            path: path.to_path_buf(),
        };
        if let Ok(conn) = db.conn() {
            let _ = conn.execute_batch("
                CREATE TABLE IF NOT EXISTS exec_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    script_id TEXT NOT NULL,
                    script_name TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    exit_code INTEGER,
                    duration_ms INTEGER,
                    output_lines INTEGER DEFAULT 0,
                    params_json TEXT NOT NULL DEFAULT '{}',
                    args_json TEXT NOT NULL DEFAULT '[]'
                );
                CREATE INDEX IF NOT EXISTS idx_exec_script ON exec_history(script_id);
                CREATE INDEX IF NOT EXISTS idx_exec_ts ON exec_history(timestamp);

                CREATE TABLE IF NOT EXISTS alert_events (
                    id TEXT PRIMARY KEY,
                    alert_type TEXT NOT NULL,
                    level TEXT NOT NULL,
                    title TEXT NOT NULL,
                    message TEXT NOT NULL,
                    service_name TEXT,
                    pid TEXT,
                    log_path TEXT,
                    summary TEXT,
                    handling TEXT,
                    evidence_json TEXT NOT NULL DEFAULT '[]',
                    suggestions_json TEXT NOT NULL DEFAULT '[]',
                    commands_json TEXT NOT NULL DEFAULT '[]',
                    first_seen TEXT NOT NULL,
                    last_seen TEXT NOT NULL,
                    occurrence_count INTEGER NOT NULL DEFAULT 1,
                    active INTEGER NOT NULL DEFAULT 1
                );
                CREATE INDEX IF NOT EXISTS idx_alert_active ON alert_events(active, level, last_seen);
                CREATE INDEX IF NOT EXISTS idx_alert_type ON alert_events(alert_type);

                CREATE TABLE IF NOT EXISTS check_configs (
                    check_id TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL DEFAULT '{}',
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS service_log_cache (
                    service_name TEXT PRIMARY KEY,
                    log_path TEXT NOT NULL,
                    source TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS metric_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    ts_ms INTEGER NOT NULL,
                    cpu_usage REAL NOT NULL DEFAULT 0,
                    memory_usage REAL NOT NULL DEFAULT 0,
                    load_one REAL NOT NULL DEFAULT 0,
                    load_ratio REAL NOT NULL DEFAULT 0,
                    rx_bytes INTEGER NOT NULL DEFAULT 0,
                    tx_bytes INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_metric_ts ON metric_history(ts_ms);

                CREATE TABLE IF NOT EXISTS rule_overrides (
                    rule_id TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL DEFAULT '{}',
                    updated_at TEXT NOT NULL
                );
            ");
            let _ = conn.execute_batch(
                "
                ALTER TABLE exec_history ADD COLUMN params_json TEXT NOT NULL DEFAULT '{}';
            ",
            );
            let _ = conn.execute_batch(
                "
                ALTER TABLE exec_history ADD COLUMN args_json TEXT NOT NULL DEFAULT '[]';
            ",
            );
            db.cleanup_metric_history();
        }
        db
    }

    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        self.conn().is_ok()
    }

    pub fn insert_exec_with_inputs(
        &self,
        script_id: &str,
        script_name: &str,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        output_lines: usize,
        params_value: &Value,
        args: &[String],
    ) {
        if let Ok(conn) = self.conn() {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let params_json =
                serde_json::to_string(params_value).unwrap_or_else(|_| "{}".to_string());
            let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string());
            let _ = conn.execute(
                "INSERT INTO exec_history (script_id, script_name, timestamp, exit_code, duration_ms, output_lines, params_json, args_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![script_id, script_name, ts, exit_code, duration_ms.map(|v| v as i64), output_lines as i64, params_json, args_json],
            );
        }
    }

    pub fn update_exec(
        &self,
        script_id: &str,
        exit_code: i32,
        duration_ms: u64,
        output_lines: usize,
    ) {
        if let Ok(conn) = self.conn() {
            let _ = conn.execute(
                "UPDATE exec_history SET exit_code = ?1, duration_ms = ?2, output_lines = ?3 WHERE id = (SELECT id FROM exec_history WHERE script_id = ?4 AND exit_code IS NULL ORDER BY id DESC LIMIT 1)",
                params![exit_code, duration_ms as i64, output_lines as i64, script_id],
            );
        }
    }

    pub fn get_history(&self, script_id: Option<&str>, limit: usize) -> Vec<ExecRecord> {
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let sql = if script_id.is_some() {
            "SELECT id, script_id, script_name, timestamp, exit_code, duration_ms, output_lines, params_json, args_json FROM exec_history WHERE script_id = ?1 ORDER BY id DESC LIMIT ?2"
        } else {
            "SELECT id, script_id, script_name, timestamp, exit_code, duration_ms, output_lines, params_json, args_json FROM exec_history ORDER BY id DESC LIMIT ?1"
        };
        let mut stmt = match conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = if let Some(sid) = script_id {
            stmt.query_map(params![sid, limit as i64], Self::map_row)
        } else {
            stmt.query_map(params![limit as i64], Self::map_row)
        };
        rows.into_iter().flatten().filter_map(|r| r.ok()).collect()
    }

    fn map_row(row: &rusqlite::Row) -> SqlResult<ExecRecord> {
        Ok(ExecRecord {
            id: row.get(0)?,
            script_id: row.get(1)?,
            script_name: row.get(2)?,
            timestamp: row.get(3)?,
            exit_code: row.get(4)?,
            duration_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
            output_lines: row.get::<_, i64>(6)? as usize,
            params: serde_json::from_str::<Value>(&row.get::<_, String>(7)?)
                .unwrap_or_else(|_| serde_json::json!({})),
            args: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(8)?)
                .unwrap_or_default(),
        })
    }

    pub fn get_stats(&self) -> (usize, usize, usize) {
        let Ok(conn) = self.conn() else {
            return (0, 0, 0);
        };
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM exec_history", [], |r| r.get(0))
            .unwrap_or(0);
        let success: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exec_history WHERE exit_code = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let failure: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exec_history WHERE exit_code IS NOT NULL AND exit_code != 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (total as usize, success as usize, failure as usize)
    }

    #[allow(dead_code)]
    pub fn get_daily_stats(&self, days: usize) -> Vec<DailyStats> {
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let offset = format!("-{} days", days);
        let Ok(mut stmt) = conn.prepare(
            "SELECT DATE(timestamp) as d, COUNT(*), SUM(CASE WHEN exit_code=0 THEN 1 ELSE 0 END), SUM(CASE WHEN exit_code IS NOT NULL AND exit_code!=0 THEN 1 ELSE 0 END) FROM exec_history WHERE timestamp >= DATE('now', ?1) GROUP BY d ORDER BY d"
        ) else { return Vec::new() };
        stmt.query_map(params![offset], |row| {
            Ok(DailyStats {
                date: row.get(0)?,
                total: row.get::<_, i64>(1)? as usize,
                success: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as usize,
                failure: row.get::<_, Option<i64>>(3)?.unwrap_or(0) as usize,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn clear_history(&self) {
        if let Ok(conn) = self.conn() {
            let _ = conn.execute("DELETE FROM exec_history", []);
        }
    }

    pub fn get_script_stats(&self, script_id: &str) -> (usize, usize, usize, Option<f64>) {
        let Ok(conn) = self.conn() else {
            return (0, 0, 0, None);
        };
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exec_history WHERE script_id = ?1",
                params![script_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let success: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exec_history WHERE script_id = ?1 AND exit_code = 0",
                params![script_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let failure: i64 = conn.query_row("SELECT COUNT(*) FROM exec_history WHERE script_id = ?1 AND exit_code IS NOT NULL AND exit_code != 0", params![script_id], |r| r.get(0)).unwrap_or(0);
        let avg_dur: Option<f64> = conn.query_row("SELECT AVG(duration_ms) FROM exec_history WHERE script_id = ?1 AND duration_ms IS NOT NULL", params![script_id], |r| r.get(0)).ok().flatten();
        (total as usize, success as usize, failure as usize, avg_dur)
    }

    pub fn upsert_active_alerts(&self, alerts: &[serde_json::Value]) {
        let Ok(mut conn) = self.conn() else { return };
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let Ok(tx) = conn.transaction() else { return };

        for alert in alerts {
            let id = alert["id"].as_str().unwrap_or("unknown").to_string();
            let alert_type = alert["type"].as_str().unwrap_or("other").to_string();
            let level = alert["level"].as_str().unwrap_or("warn").to_string();
            let title = alert["title"].as_str().unwrap_or("").to_string();
            let message = alert["message"].as_str().unwrap_or("").to_string();
            let service_name = alert["service_name"].as_str().map(|s| s.to_string());
            let pid = alert["pid"].as_str().map(|s| s.to_string());
            let log_path = alert["log_path"].as_str().map(|s| s.to_string());
            let summary = alert["summary"]
                .as_str()
                .map(|s| s.to_string())
                .or(Some(message.clone()));
            let handling = alert["handling"].as_str().map(|s| s.to_string());
            let evidence_json = serde_json::to_string(
                alert["evidence"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .as_slice(),
            )
            .unwrap_or_else(|_| "[]".to_string());
            let suggestions_json = serde_json::to_string(
                alert["suggestions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .as_slice(),
            )
            .unwrap_or_else(|_| "[]".to_string());
            let commands_json = serde_json::to_string(
                alert["commands"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .as_slice(),
            )
            .unwrap_or_else(|_| "[]".to_string());

            let _ = tx.execute(
                "INSERT INTO alert_events
                    (id, alert_type, level, title, message, service_name, pid, log_path, summary, handling, evidence_json, suggestions_json, commands_json, first_seen, last_seen, occurrence_count, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, 1, 1)
                 ON CONFLICT(id) DO UPDATE SET
                    alert_type = excluded.alert_type,
                    level = excluded.level,
                    title = excluded.title,
                    message = excluded.message,
                    service_name = excluded.service_name,
                    pid = excluded.pid,
                    log_path = excluded.log_path,
                    summary = excluded.summary,
                    handling = excluded.handling,
                    evidence_json = excluded.evidence_json,
                    suggestions_json = excluded.suggestions_json,
                    commands_json = excluded.commands_json,
                    last_seen = excluded.last_seen,
                    occurrence_count = alert_events.occurrence_count + 1,
                    active = 1",
                params![
                    id,
                    alert_type,
                    level,
                    title,
                    message,
                    service_name,
                    pid,
                    log_path,
                    summary,
                    handling,
                    evidence_json,
                    suggestions_json,
                    commands_json,
                    now,
                ],
            );
        }

        let _ = tx.commit();
    }

    pub fn get_alerts(&self, active_only: bool, limit: usize) -> Vec<AlertRecord> {
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let sql = if active_only {
            "SELECT id, alert_type, level, title, message, service_name, pid, log_path, summary, handling, evidence_json, suggestions_json, commands_json, first_seen, last_seen, occurrence_count, active
             FROM alert_events WHERE active = 1
             ORDER BY CASE level WHEN 'error' THEN 0 WHEN 'warn' THEN 1 ELSE 2 END, last_seen DESC LIMIT ?1"
        } else {
            "SELECT id, alert_type, level, title, message, service_name, pid, log_path, summary, handling, evidence_json, suggestions_json, commands_json, first_seen, last_seen, occurrence_count, active
             FROM alert_events
             ORDER BY CASE level WHEN 'error' THEN 0 WHEN 'warn' THEN 1 ELSE 2 END, last_seen DESC LIMIT ?1"
        };
        let Ok(mut stmt) = conn.prepare(sql) else {
            return Vec::new();
        };
        stmt.query_map(params![limit as i64], |row| {
            Ok(AlertRecord {
                id: row.get(0)?,
                alert_type: row.get(1)?,
                level: row.get(2)?,
                title: row.get(3)?,
                message: row.get(4)?,
                service_name: row.get(5)?,
                pid: row.get(6)?,
                log_path: row.get(7)?,
                summary: row.get(8)?,
                handling: row.get(9)?,
                evidence_json: row.get(10)?,
                suggestions_json: row.get(11)?,
                commands_json: row.get(12)?,
                first_seen: row.get(13)?,
                last_seen: row.get(14)?,
                occurrence_count: row.get::<_, i64>(15)? as usize,
                active: row.get::<_, i64>(16)? != 0,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn clear_alerts(&self) -> bool {
        let Ok(conn) = self.conn() else {
            return false;
        };
        conn.execute("DELETE FROM alert_events", []).is_ok()
    }

    pub fn get_check_config(&self, check_id: &str) -> Option<CheckConfigRecord> {
        let Ok(conn) = self.conn() else {
            return None;
        };
        conn.query_row(
            "SELECT check_id, value_json, updated_at FROM check_configs WHERE check_id = ?1",
            params![check_id],
            |row| {
                let value_json: String = row.get(1)?;
                Ok(CheckConfigRecord {
                    check_id: row.get(0)?,
                    value: serde_json::from_str(&value_json)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    updated_at: row.get(2)?,
                })
            },
        )
        .ok()
    }

    pub fn list_check_configs(&self) -> Vec<CheckConfigRecord> {
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT check_id, value_json, updated_at FROM check_configs ORDER BY check_id ASC",
        ) else {
            return Vec::new();
        };
        stmt.query_map([], |row| {
            let value_json: String = row.get(1)?;
            Ok(CheckConfigRecord {
                check_id: row.get(0)?,
                value: serde_json::from_str(&value_json).unwrap_or_else(|_| serde_json::json!({})),
                updated_at: row.get(2)?,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn save_check_config(&self, check_id: &str, value: &serde_json::Value) -> bool {
        let Ok(conn) = self.conn() else {
            return false;
        };
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let value_json = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "INSERT INTO check_configs (check_id, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(check_id) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at",
            params![check_id, value_json, now],
        )
        .is_ok()
    }

    pub fn get_service_log_cache(&self, service_name: &str) -> Option<(String, String, String)> {
        let Ok(conn) = self.conn() else {
            return None;
        };
        conn.query_row(
            "SELECT log_path, source, updated_at FROM service_log_cache WHERE service_name = ?1",
            params![service_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
    }

    pub fn save_service_log_cache(&self, service_name: &str, log_path: &str, source: &str) -> bool {
        if service_name.trim().is_empty() || log_path.trim().is_empty() {
            return false;
        }
        let Ok(conn) = self.conn() else {
            return false;
        };
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        conn.execute(
            "INSERT INTO service_log_cache (service_name, log_path, source, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(service_name) DO UPDATE SET
                log_path = excluded.log_path,
                source = excluded.source,
                updated_at = excluded.updated_at",
            params![service_name, log_path, source, now],
        )
        .is_ok()
    }

    pub fn insert_metric_point(
        &self,
        cpu_usage: f64,
        memory_usage: f64,
        load_one: f64,
        load_ratio: f64,
        rx_bytes: i64,
        tx_bytes: i64,
    ) {
        let Ok(conn) = self.conn() else { return };
        let now = chrono::Local::now();
        let ts = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let ts_ms = now.timestamp_millis();
        let _ = conn.execute(
            "INSERT INTO metric_history (timestamp, ts_ms, cpu_usage, memory_usage, load_one, load_ratio, rx_bytes, tx_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                ts,
                ts_ms,
                cpu_usage,
                memory_usage,
                load_one,
                load_ratio,
                rx_bytes,
                tx_bytes
            ],
        );
        if ts_ms % 120_000 < 15_000 {
            self.cleanup_metric_history();
        }
    }

    pub fn get_metric_history(&self, minutes: u64) -> Vec<MetricPoint> {
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let minutes = minutes.clamp(1, 120);
        let cutoff = chrono::Local::now().timestamp_millis() - minutes as i64 * 60_000;
        let Ok(mut stmt) = conn.prepare(
            "SELECT timestamp, ts_ms, cpu_usage, memory_usage, load_one, load_ratio, rx_bytes, tx_bytes
             FROM metric_history WHERE ts_ms >= ?1 ORDER BY ts_ms ASC",
        ) else {
            return Vec::new();
        };
        stmt.query_map(params![cutoff], |row| {
            Ok(MetricPoint {
                timestamp: row.get(0)?,
                ts_ms: row.get(1)?,
                cpu_usage: row.get(2)?,
                memory_usage: row.get(3)?,
                load_one: row.get(4)?,
                load_ratio: row.get(5)?,
                rx_bytes: row.get(6)?,
                tx_bytes: row.get(7)?,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn cleanup_metric_history(&self) {
        if let Ok(conn) = self.conn() {
            let cutoff = chrono::Local::now().timestamp_millis() - 2 * 60 * 60 * 1000;
            let _ = conn.execute(
                "DELETE FROM metric_history WHERE ts_ms < ?1",
                params![cutoff],
            );
        }
    }

    pub fn get_rule_overrides(&self) -> Vec<RuleOverrideRecord> {
        let Ok(conn) = self.conn() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT rule_id, value_json, updated_at FROM rule_overrides ORDER BY rule_id ASC",
        ) else {
            return Vec::new();
        };
        stmt.query_map([], |row| {
            let value_json: String = row.get(1)?;
            Ok(RuleOverrideRecord {
                rule_id: row.get(0)?,
                value: serde_json::from_str(&value_json).unwrap_or_else(|_| serde_json::json!({})),
                updated_at: row.get(2)?,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn save_rule_override(&self, rule_id: &str, value: &serde_json::Value) -> bool {
        let Ok(conn) = self.conn() else {
            return false;
        };
        let updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let value_json = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "INSERT INTO rule_overrides (rule_id, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(rule_id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![rule_id, value_json, updated_at],
        )
        .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "dm-{}-{}-{}.db",
            name,
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ))
    }

    #[test]
    fn check_config_round_trips_and_updates() {
        let path = temp_db_path("check-config");
        let db = Database::open(&path);

        let first = serde_json::json!({
            "host": "127.0.0.1",
            "port": "9200",
            "username": "elastic"
        });
        assert!(db.save_check_config("elasticsearch", &first));
        let saved = db.get_check_config("elasticsearch").expect("config saved");
        assert_eq!(saved.check_id, "elasticsearch");
        assert_eq!(saved.value["host"], "127.0.0.1");
        assert_eq!(saved.value["port"], "9200");

        let updated = serde_json::json!({
            "host": "10.0.0.8",
            "port": "9201",
            "log_path": "/var/log/elasticsearch/es.log"
        });
        assert!(db.save_check_config("elasticsearch", &updated));
        let saved = db
            .get_check_config("elasticsearch")
            .expect("config updated");
        assert_eq!(saved.value["host"], "10.0.0.8");
        assert_eq!(saved.value["log_path"], "/var/log/elasticsearch/es.log");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn exec_history_preserves_params_and_args() {
        let path = temp_db_path("exec-inputs");
        let db = Database::open(&path);
        let params_value = serde_json::json!({
            "target": "nginx",
            "dry_run": "true"
        });
        let args = vec![
            "--verbose".to_string(),
            "/var/log/nginx/error.log".to_string(),
        ];

        db.insert_exec_with_inputs(
            "restart-nginx",
            "Restart Nginx",
            None,
            None,
            0,
            &params_value,
            &args,
        );
        db.update_exec("restart-nginx", 0, 1234, 7);

        let records = db.get_history(Some("restart-nginx"), 5);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].params["target"], "nginx");
        assert_eq!(records[0].params["dry_run"], "true");
        assert_eq!(records[0].args, args);
        assert_eq!(records[0].exit_code, Some(0));
        assert_eq!(records[0].duration_ms, Some(1234));
        assert_eq!(records[0].output_lines, 7);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn clear_alerts_removes_active_and_history_records() {
        let path = temp_db_path("clear-alerts");
        let db = Database::open(&path);
        let alerts = vec![
            serde_json::json!({
                "id": "test-alert-1",
                "type": "service",
                "level": "warn",
                "title": "服务警告",
                "message": "服务存在异常",
                "summary": "服务存在异常",
                "evidence": ["unit failed"],
                "suggestions": ["查看服务日志"],
                "commands": ["systemctl status demo"]
            }),
            serde_json::json!({
                "id": "test-alert-2",
                "type": "log",
                "level": "error",
                "title": "日志错误",
                "message": "出现错误日志",
                "summary": "出现错误日志"
            }),
        ];
        db.upsert_active_alerts(&alerts);
        assert_eq!(db.get_alerts(true, 10).len(), 2);
        assert_eq!(db.get_alerts(false, 10).len(), 2);

        assert!(db.clear_alerts());
        assert!(db.get_alerts(true, 10).is_empty());
        assert!(db.get_alerts(false, 10).is_empty());

        let _ = std::fs::remove_file(path);
    }
}
