use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// 执行日志消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    /// 消息类型: log, error, done
    pub msg_type: String,
    /// 时间戳
    pub timestamp: String,
    /// 日志内容
    pub line: String,
    /// 退出码 (仅 done 消息)
    pub exit_code: Option<i32>,
    /// 是否超时
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
    /// 耗时（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

/// 根据文件扩展名解析解释器
pub fn resolve_interpreter(script_path: &Path) -> (String, Vec<String>) {
    let script_str = script_path.to_string_lossy().to_string();
    match script_path.extension().and_then(|e| e.to_str()) {
        Some("sh") | Some("bash") => ("bash".to_string(), vec![script_str]),
        Some("zsh") => ("zsh".to_string(), vec![script_str]),
        Some("ksh") => ("ksh".to_string(), vec![script_str]),
        Some("py") | Some("python") => ("python3".to_string(), vec![script_str]),
        Some("js") | Some("mjs") => ("node".to_string(), vec![script_str]),
        Some("pl") | Some("perl") => ("perl".to_string(), vec![script_str]),
        Some("rb") => ("ruby".to_string(), vec![script_str]),
        Some("lua") => ("lua".to_string(), vec![script_str]),
        Some("php") => ("php".to_string(), vec![script_str]),
        Some("awk") => ("awk".to_string(), vec!["-f".to_string(), script_str]),
        Some("expect") | Some("exp") => ("expect".to_string(), vec![script_str]),
        _ => (script_str, vec![]),
    }
}

pub fn system_environment() -> HashMap<String, String> {
    let mut envs: HashMap<String, String> = std::env::vars().collect();
    for path in ["/etc/environment", "/etc/default/locale"] {
        merge_env_file(&mut envs, path);
    }
    envs.entry("PATH".to_string()).or_insert_with(|| {
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()
    });
    envs
}

pub fn parameter_environment(params: &HashMap<String, String>) -> HashMap<String, String> {
    let mut envs = HashMap::new();
    if params.is_empty() {
        return envs;
    }
    for (key, value) in params {
        let name = normalize_env_key(key);
        if name.is_empty() {
            continue;
        }
        envs.insert(format!("DM_PARAM_{}", name), value.clone());
        envs.insert(format!("PARAM_{}", name), value.clone());
    }
    if let Ok(raw) = serde_json::to_string(params) {
        envs.insert("DM_SCRIPT_PARAMS_JSON".to_string(), raw);
    }
    envs
}

fn normalize_env_key(key: &str) -> String {
    let mut out = String::new();
    for c in key.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else if c == '_' || c == '-' || c == '.' {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn merge_env_file(envs: &mut HashMap<String, String>, path: &str) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || envs.contains_key(key) {
            continue;
        }
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        envs.insert(key.to_string(), value);
    }
}

/// 执行脚本，返回日志接收通道
///
/// `timeout_secs` 为 None 或 Some(0) 表示无超时限制
pub async fn execute_script(
    script_path: &Path,
    args: Vec<String>,
    envs: HashMap<String, String>,
    timeout_secs: Option<u64>,
) -> Result<(mpsc::Receiver<LogMessage>, tokio::task::JoinHandle<i32>)> {
    let (tx, rx) = mpsc::channel(1024);
    let script_path = script_path.to_path_buf();
    let timeout = timeout_secs.filter(|&s| s > 0).map(Duration::from_secs);

    let handle = tokio::spawn(async move {
        let started = std::time::Instant::now();
        let result = run_script(&script_path, &args, &envs, tx.clone(), timeout).await;
        let elapsed = started.elapsed().as_millis() as u64;

        match result {
            Ok(RunOutcome::Completed(code)) => {
                let _ = tx
                    .send(LogMessage {
                        msg_type: "done".to_string(),
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%d %H:%M:%S%.3f")
                            .to_string(),
                        line: if code == 0 {
                            format!("执行完成 [用时 {}ms]", elapsed)
                        } else {
                            format!("执行失败，退出码: {} [用时 {}ms]", code, elapsed)
                        },
                        exit_code: Some(code),
                        timed_out: None,
                        elapsed_ms: Some(elapsed),
                    })
                    .await;
                code
            }
            Ok(RunOutcome::TimedOut) => {
                let _ = tx
                    .send(LogMessage {
                        msg_type: "done".to_string(),
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%d %H:%M:%S%.3f")
                            .to_string(),
                        line: format!(
                            "⏱ 执行超时（>{}s），已强制终止 [用时 {}ms]",
                            timeout.map(|d| d.as_secs()).unwrap_or(0),
                            elapsed
                        ),
                        exit_code: Some(124),
                        timed_out: Some(true),
                        elapsed_ms: Some(elapsed),
                    })
                    .await;
                124
            }
            Err(e) => {
                let _ = tx
                    .send(LogMessage {
                        msg_type: "error".to_string(),
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%d %H:%M:%S%.3f")
                            .to_string(),
                        line: format!("执行错误: {}", e),
                        exit_code: Some(1),
                        timed_out: None,
                        elapsed_ms: Some(elapsed),
                    })
                    .await;
                1
            }
        }
    });

    Ok((rx, handle))
}

enum RunOutcome {
    Completed(i32),
    TimedOut,
}

async fn run_script(
    script_path: &Path,
    args: &[String],
    envs: &HashMap<String, String>,
    tx: mpsc::Sender<LogMessage>,
    timeout: Option<Duration>,
) -> Result<RunOutcome> {
    let (cmd, mut cmd_args) = resolve_interpreter(script_path);
    cmd_args.extend(args.iter().cloned());

    let mut child = Command::new(&cmd)
        .args(&cmd_args)
        .envs(system_environment())
        .envs(envs)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("无法启动进程: {}", cmd))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("无法获取 stdout 管道"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("无法获取 stderr 管道"))?;

    let tx_out = tx.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx_out
                .send(LogMessage {
                    msg_type: "log".to_string(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%d %H:%M:%S%.3f")
                        .to_string(),
                    line,
                    exit_code: None,
                    timed_out: None,
                    elapsed_ms: None,
                })
                .await;
        }
    });

    let tx_err = tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx_err
                .send(LogMessage {
                    msg_type: "error".to_string(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%d %H:%M:%S%.3f")
                        .to_string(),
                    line,
                    exit_code: None,
                    timed_out: None,
                    elapsed_ms: None,
                })
                .await;
        }
    });

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let status = if let Some(d) = timeout {
        match tokio::time::timeout(d, child.wait()).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Ok(RunOutcome::TimedOut);
            }
        }
    } else {
        child.wait().await?
    };

    let exit_code = status.code().unwrap_or(-1);
    Ok(RunOutcome::Completed(exit_code))
}

#[cfg(test)]
mod tests {
    use super::{parameter_environment, resolve_interpreter, system_environment};
    use std::path::Path;

    #[test]
    fn resolves_common_executable_script_types() {
        let cases = [
            ("maint.sh", "bash"),
            ("maint.bash", "bash"),
            ("maint.py", "python3"),
            ("maint.python", "python3"),
            ("maint.pl", "perl"),
            ("maint.perl", "perl"),
            ("maint.expect", "expect"),
            ("maint.awk", "awk"),
        ];
        for (file, expected) in cases {
            let (cmd, args) = resolve_interpreter(Path::new(file));
            assert_eq!(cmd, expected);
            assert!(args.iter().any(|arg| arg.contains(file)));
        }
    }

    #[test]
    fn system_environment_always_contains_path() {
        let envs = system_environment();
        assert!(envs.get("PATH").is_some_and(|value| !value.is_empty()));
    }

    #[test]
    fn parameter_environment_exports_shell_friendly_names() {
        let mut params = std::collections::HashMap::new();
        params.insert("service-name".to_string(), "nginx".to_string());
        params.insert("dry_run".to_string(), "true".to_string());

        let envs = parameter_environment(&params);

        assert_eq!(
            envs.get("DM_PARAM_SERVICE_NAME").map(String::as_str),
            Some("nginx")
        );
        assert_eq!(envs.get("PARAM_DRY_RUN").map(String::as_str), Some("true"));
        assert!(envs
            .get("DM_SCRIPT_PARAMS_JSON")
            .is_some_and(|raw| raw.contains("service-name")));
    }
}
