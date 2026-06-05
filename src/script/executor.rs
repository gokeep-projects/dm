use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        Some("sh") => ("bash".to_string(), vec![script_str]),
        Some("py") => ("python3".to_string(), vec![script_str]),
        Some("js") => ("node".to_string(), vec![script_str]),
        Some("pl") => ("perl".to_string(), vec![script_str]),
        Some("rb") => ("ruby".to_string(), vec![script_str]),
        Some("lua") => ("lua".to_string(), vec![script_str]),
        _ => (script_str, vec![]),
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
