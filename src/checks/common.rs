use super::{CheckStatus, Item, Section};
use crate::config::Config;
use crate::db::Database;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EndpointConfig {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub service_prefix: String,
    #[serde(default)]
    pub config_path: String,
    #[serde(default)]
    pub data_path: String,
    #[serde(default)]
    pub log_path: String,
    #[serde(default)]
    pub program_path: String,
    #[serde(default)]
    pub extra: serde_json::Value,
}

impl EndpointConfig {
    pub fn address(&self, default_host: &str, default_port: &str) -> (String, String) {
        let host = if self.host.trim().is_empty() {
            default_host.to_string()
        } else {
            self.host.trim().to_string()
        };
        let port = if self.port.trim().is_empty() {
            default_port.to_string()
        } else {
            self.port.trim().to_string()
        };
        (host, port)
    }

    pub fn base_url(&self, default_scheme: &str, default_host: &str, default_port: &str) -> String {
        if !self.url.trim().is_empty() {
            return self.url.trim().trim_end_matches('/').to_string();
        }
        let (host, port) = self.address(default_host, default_port);
        format!("{}://{}:{}", default_scheme, host, port)
    }
}

pub fn load_endpoint_config(check_id: &str) -> EndpointConfig {
    let cfg = Config::load();
    crate::config::ensure_user_dirs(&cfg);
    let db = Database::open(&crate::config::db_path(&cfg));
    db.get_check_config(check_id)
        .and_then(|record| serde_json::from_value(record.value).ok())
        .unwrap_or_default()
}

pub fn command_output(cmd: &str, args: &[&str]) -> Option<String> {
    command_output_timeout(cmd, args, Duration::from_secs(5))
}

pub fn command_output_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        if child.try_wait().ok().flatten().is_some() {
            let output = child.wait_with_output().ok()?;
            let mut text = String::new();
            text.push_str(&String::from_utf8_lossy(&output.stdout));
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                if !text.trim().is_empty() {
                    text.push('\n');
                }
                text.push_str(&stderr);
            }
            return Some(text.trim().to_string());
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[allow(dead_code)]
pub fn command_output_unbounded(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.trim().is_empty() {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    Some(text.trim().to_string())
}

pub fn shell_output(script: &str) -> Option<String> {
    command_output("sh", &["-c", script])
}

pub fn find_command(cmd: &str) -> Option<String> {
    shell_output(&format!("command -v {} 2>/dev/null", shell_escape(cmd)))
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.lines().next().unwrap_or("").to_string())
}

pub fn process_rows(patterns: &[&str]) -> Vec<Vec<String>> {
    let Some(out) = shell_output("ps -eo pid,ppid,user,stat,%cpu,%mem,lstart,args --no-headers")
    else {
        return Vec::new();
    };
    let lower_patterns: Vec<String> = patterns.iter().map(|p| p.to_lowercase()).collect();
    let mut rows = Vec::new();
    for line in out.lines() {
        let lower = line.to_lowercase();
        if !lower_patterns.iter().any(|p| lower.contains(p)) || lower.contains("grep ") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let pid = parts[0].to_string();
        let ppid = parts[1].to_string();
        let user = parts[2].to_string();
        let stat = parts[3].to_string();
        let cpu = parts[4].to_string();
        let mem = parts[5].to_string();
        let cmd = parts[10..].join(" ");
        rows.push(vec![pid, ppid, user, stat, cpu, mem, truncate(&cmd, 160)]);
    }
    rows
}

pub fn listen_rows(patterns: &[&str], ports: &[&str]) -> Vec<Vec<String>> {
    let Some(out) = shell_output("ss -ltnp 2>/dev/null || netstat -ltnp 2>/dev/null") else {
        return Vec::new();
    };
    let lower_patterns: Vec<String> = patterns.iter().map(|p| p.to_lowercase()).collect();
    let mut rows = Vec::new();
    for line in out.lines() {
        let lower = line.to_lowercase();
        let match_proc = lower_patterns.iter().any(|p| lower.contains(p));
        let match_port = ports.iter().any(|p| line.contains(&format!(":{}", p)));
        if !match_proc && !match_port {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 5 {
            rows.push(vec![
                cols.get(0).unwrap_or(&"-").to_string(),
                cols.get(3).unwrap_or(&"-").to_string(),
                cols.get(4).unwrap_or(&"-").to_string(),
                cols.last().unwrap_or(&"-").to_string(),
            ]);
        }
    }
    rows
}

pub fn first_existing(paths: &[&str]) -> Option<PathBuf> {
    paths.iter().map(PathBuf::from).find(|p| p.exists())
}

pub fn read_file_limited(path: &Path, max_bytes: usize) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let slice = if data.len() > max_bytes {
        &data[..max_bytes]
    } else {
        &data
    };
    Some(String::from_utf8_lossy(slice).to_string())
}

pub fn tail_lines(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines
}

pub fn grep_error_lines(path: &Path, max_lines: usize) -> Vec<String> {
    let keywords = [
        "error",
        "exception",
        "fatal",
        "panic",
        "failed",
        "timeout",
        "denied",
        "corrupt",
        "oom",
        "unavailable",
        "refused",
    ];
    tail_lines(path, 2000)
        .into_iter()
        .filter(|line| {
            let lower = line.to_lowercase();
            keywords.iter().any(|kw| lower.contains(kw))
        })
        .rev()
        .take(max_lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn log_section(title: &str, path: Option<PathBuf>, max_lines: usize) -> Section {
    let mut items = Vec::new();
    if let Some(path) = path {
        items.push(Item::Label {
            key: "日志路径".to_string(),
            value: path.display().to_string(),
            status: Some(if path.exists() { "ok" } else { "warn" }.to_string()),
        });
        let errors = grep_error_lines(&path, max_lines);
        if errors.is_empty() {
            items.push(Item::Success {
                text: "最近日志未匹配到关键异常关键词".to_string(),
            });
        } else {
            items.push(Item::Table {
                headers: vec!["序号".to_string(), "异常日志".to_string()],
                rows: errors
                    .into_iter()
                    .enumerate()
                    .map(|(i, line)| vec![(i + 1).to_string(), truncate(&line, 260)])
                    .collect(),
                status: Some("warn".to_string()),
            });
        }
    } else {
        items.push(Item::Warning {
            text: "未推断到日志路径，请在检查配置中补充 log_path".to_string(),
        });
    }
    Section {
        title: title.to_string(),
        icon: Some("LOG".to_string()),
        description: Some(
            "匹配 error/exception/fatal/panic/failed/timeout/corrupt 等关键异常".to_string(),
        ),
        items,
    }
}

pub fn config_preview_section(title: &str, path: Option<PathBuf>) -> Section {
    let mut items = Vec::new();
    if let Some(path) = path {
        items.push(Item::Label {
            key: "配置路径".to_string(),
            value: path.display().to_string(),
            status: Some(if path.exists() { "ok" } else { "warn" }.to_string()),
        });
        if let Some(content) = read_file_limited(&path, 12 * 1024) {
            let rows = content
                .lines()
                .take(160)
                .enumerate()
                .map(|(i, line)| vec![(i + 1).to_string(), truncate(line, 220)])
                .collect();
            items.push(Item::Table {
                headers: vec!["行".to_string(), "内容".to_string()],
                rows,
                status: None,
            });
        }
    } else {
        items.push(Item::Warning {
            text: "未推断到配置路径，请在检查配置中补充 config_path".to_string(),
        });
    }
    Section {
        title: title.to_string(),
        icon: Some("CONF".to_string()),
        description: Some("配置内容默认截断展示，可在页面展开表格查看".to_string()),
        items,
    }
}

pub fn table_section(
    title: &str,
    headers: Vec<&str>,
    rows: Vec<Vec<String>>,
    empty: &str,
) -> Section {
    let items = if rows.is_empty() {
        vec![Item::Info {
            text: empty.to_string(),
        }]
    } else {
        vec![Item::Table {
            headers: headers.into_iter().map(|v| v.to_string()).collect(),
            rows,
            status: None,
        }]
    };
    Section {
        title: title.to_string(),
        icon: None,
        description: None,
        items,
    }
}

pub fn label(key: &str, value: impl Into<String>, status: Option<&str>) -> Item {
    Item::Label {
        key: key.to_string(),
        value: value.into(),
        status: status.map(|v| v.to_string()),
    }
}

pub fn status_from_bool(ok: bool) -> CheckStatus {
    if ok {
        CheckStatus::Ok
    } else {
        CheckStatus::Warn
    }
}

pub fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in value.chars().enumerate() {
        if i >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

pub fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:".contains(c))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
