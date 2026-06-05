use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{interval, Duration};

use crate::script::{self, executor};
use crate::web::api::AppState;

const WS_PING_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
pub struct WsRunRequest {
    pub action: String,
    pub params: HashMap<String, String>,
    pub args: Vec<String>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct WsLogMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: String,
    pub line: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, id, state))
}

pub async fn ws_dashboard_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_dashboard_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, script_id: String, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    send_message(&mut sender, "connected", "已连接到脚本", None).await;

    let mut ping_timer = interval(WS_PING_INTERVAL);
    ping_timer.tick().await;

    loop {
        tokio::select! {
            _ = ping_timer.tick() => {
                if sender.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: String = text.into();
                        let req: WsRunRequest = match serde_json::from_str(&text_str) {
                            Ok(r) => r,
                            Err(e) => {
                                send_message(&mut sender, "error", &format!("请求解析错误: {}", e), None).await;
                                continue;
                            }
                        };

                        if req.action != "run" {
                            continue;
                        }

                        let dirs = crate::config::all_script_dirs(&state.config);
                        let script = match script::find_script(&dirs, &script_id) {
                            Ok(Some(s)) => s,
                            Ok(None) => {
                                send_message(&mut sender, "error", &format!("脚本不存在: {}", script_id), None).await;
                                break;
                            }
                            Err(e) => {
                                send_message(&mut sender, "error", &format!("查找脚本错误: {}", e), None).await;
                                break;
                            }
                        };

                        let mut exec_args: Vec<String> = Vec::new();
                        for (k, v) in &req.params {
                            if !v.is_empty() {
                                exec_args.push(format!("--{}={}", k, v));
                            }
                        }
                        exec_args.extend(req.args.iter().cloned());

                        let mut envs = HashMap::new();
                        envs.insert("COLUMNS".to_string(), "300".to_string());
                        state.db.insert_exec(&script.id, &script.name, None, None, 0);

                        if let Some(result) = run_structured_script(&script.id).await {
                            let elapsed = result
                                .get("duration_ms")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let status = result
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("ok");
                            let exit_code = if status == "error" { 1 } else { 0 };
                            let line_count = result
                                .get("sections")
                                .and_then(|v| v.as_array())
                                .map(|sections| sections.len())
                                .unwrap_or(0);
                            state.db.update_exec(&script_id, exit_code, elapsed, line_count);

                            let result_msg = WsLogMessage {
                                msg_type: "result".to_string(),
                                timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                                line: serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
                                exit_code: Some(exit_code),
                                elapsed_ms: Some(elapsed),
                            };
                            if let Ok(json) = serde_json::to_string(&result_msg) {
                                let _ = sender.send(Message::Text(json.into())).await;
                            }
                            break;
                        }

                        match executor::execute_script(&script.path, exec_args, envs, req.timeout).await {
                            Ok((mut rx, handle)) => {
                                let mut final_exit: Option<i32> = None;
                                let mut final_elapsed: u64 = 0;
                                let mut line_count: usize = 0;
                                let mut output_lines: Vec<String> = Vec::new();

                                let log_dir = state.config.log_dir.join(&script_id);
                                let _ = std::fs::create_dir_all(&log_dir);
                                let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                                let log_path = log_dir.join(format!("{}.log", ts));
                                let mut log_file = std::fs::File::create(&log_path).ok();

                                while let Some(log_msg) = rx.recv().await {
                                    if log_msg.msg_type == "done" {
                                        final_exit = log_msg.exit_code;
                                        final_elapsed = log_msg.elapsed_ms.unwrap_or(0);
                                    } else {
                                        line_count += 1;
                                        output_lines.push(log_msg.line.clone());
                                    }

                                    if let Some(ref mut f) = log_file {
                                        use std::io::Write;
                                        let _ = writeln!(f, "[{}] {}", log_msg.timestamp, log_msg.line);
                                    }

                                    if log_msg.msg_type == "done" {
                                        break;
                                    }
                                }
                                let _ = handle.await;
                                if let Some(code) = final_exit {
                                    state.db.update_exec(&script_id, code, final_elapsed, line_count);

                                    let full_output = output_lines.join("\n");
                                    let result_json = try_parse_json_output(&full_output, code, final_elapsed);

                                    let result_msg = WsLogMessage {
                                        msg_type: "result".to_string(),
                                        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                                        line: result_json,
                                        exit_code: Some(code),
                                        elapsed_ms: Some(final_elapsed),
                                    };

                                    if let Ok(json) = serde_json::to_string(&result_msg) {
                                        let _ = sender.send(Message::Text(json.into())).await;
                                    }
                                }
                            }
                            Err(e) => {
                                send_message(&mut sender, "error", &format!("启动脚本失败: {}", e), None).await;
                            }
                        }
                        break;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                }
            }
        }
    }
}

async fn send_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    msg_type: &str,
    line: &str,
    exit_code: Option<i32>,
) {
    let msg = WsLogMessage {
        msg_type: msg_type.to_string(),
        timestamp: chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
        line: line.to_string(),
        exit_code,
        elapsed_ms: None,
    };
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }
}

fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == 'm' {
                    break;
                }
            }
        } else if c == '\x08' {
            continue;
        } else {
            result.push(c);
        }
    }
    result
}

fn clean_terminal_output(input: &str) -> String {
    let stripped = strip_ansi_codes(input);
    let mut result = String::with_capacity(stripped.len());
    for line in stripped.lines() {
        let cleaned = line
            .replace("╔", "+")
            .replace("╗", "+")
            .replace("╚", "+")
            .replace("╝", "+")
            .replace("═", "-")
            .replace("║", "|")
            .replace("┌", "+")
            .replace("┐", "+")
            .replace("└", "+")
            .replace("┘", "+")
            .replace("─", "-")
            .replace("│", "|")
            .replace("├", "+")
            .replace("┤", "+")
            .replace("┬", "+")
            .replace("┴", "+")
            .replace("┼", "+");
        result.push_str(&cleaned);
        result.push('\n');
    }
    result.trim().to_string()
}

fn strip_markdown_fence(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with("```") {
        return None;
    }

    let mut lines = trimmed.lines();
    lines.next()?;
    let mut body: Vec<&str> = lines.collect();
    if body.last().is_some_and(|line| line.trim() == "```") {
        body.pop();
    }
    Some(body.join("\n").trim().to_string())
}

fn fenced_blocks(input: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut in_fence = false;

    for line in input.lines() {
        if line.trim_start().starts_with("```") {
            if in_fence {
                blocks.push(current.join("\n").trim().to_string());
                current.clear();
                in_fence = false;
            } else {
                in_fence = true;
            }
            continue;
        }

        if in_fence {
            current.push(line);
        }
    }

    blocks
}

fn extract_balanced_json(input: &str) -> Option<String> {
    let chars: Vec<char> = input.chars().collect();
    for start in 0..chars.len() {
        let open = chars[start];
        let close = match open {
            '{' => '}',
            '[' => ']',
            _ => continue,
        };

        let mut stack = vec![close];
        let mut in_string = false;
        let mut escape = false;

        for end in start + 1..chars.len() {
            let c = chars[end];
            if in_string {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }

            match c {
                '"' => in_string = true,
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' => {
                    if stack.pop() != Some(c) {
                        break;
                    }
                    if stack.is_empty() {
                        return Some(chars[start..=end].iter().collect::<String>());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn parse_json_candidate(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    if let Some(unfenced) = strip_markdown_fence(trimmed) {
        if serde_json::from_str::<serde_json::Value>(&unfenced).is_ok() {
            return Some(unfenced);
        }
    }

    if let Some(extracted) = extract_balanced_json(trimmed) {
        if serde_json::from_str::<serde_json::Value>(&extracted).is_ok() {
            return Some(extracted);
        }
    }

    None
}

async fn run_structured_script(script_id: &str) -> Option<serde_json::Value> {
    if !matches!(
        script_id,
        "system"
            | "security"
            | "network"
            | "resource"
            | "service"
            | "environment"
            | "container"
            | "middleware"
            | "schedule"
            | "smart-check"
            | "service-manage"
    ) {
        return None;
    }

    let id = script_id.to_string();
    tokio::task::spawn_blocking(move || crate::checks::run_check(&id))
        .await
        .ok()
        .flatten()
        .and_then(|result| serde_json::to_value(result).ok())
}

fn json_value_to_result(
    value: serde_json::Value,
    exit_code: i32,
    elapsed_ms: u64,
) -> serde_json::Value {
    if value.get("sections").is_some() {
        return value;
    }

    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .filter(|s| matches!(*s, "ok" | "warn" | "error" | "info"))
        .unwrap_or(if exit_code == 0 { "ok" } else { "error" });
    let mut summary_items = vec![
        serde_json::json!({"type": "label", "key": "状态", "value": if exit_code == 0 { "成功" } else { "失败" }, "status": status}),
        serde_json::json!({"type": "label", "key": "退出码", "value": exit_code.to_string()}),
        serde_json::json!({"type": "label", "key": "耗时", "value": format!("{}ms", elapsed_ms)}),
    ];

    let mut data_items = Vec::new();
    match &value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                if key == "stdout" || key == "stderr" {
                    continue;
                }
                if val.is_string() || val.is_number() || val.is_boolean() || val.is_null() {
                    data_items.push(serde_json::json!({
                        "type": "label",
                        "key": key,
                        "value": scalar_json_display(val),
                    }));
                }
            }

            if let Some(stdout) = map
                .get("stdout")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                data_items.push(serde_json::json!({"type": "info", "text": stdout.trim()}));
            }
            if let Some(stderr) = map
                .get("stderr")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                data_items.push(serde_json::json!({"type": "error", "text": stderr.trim()}));
            }
            for (key, val) in map {
                if val.is_array() || val.is_object() {
                    data_items.push(json_complex_item(key, val));
                }
            }
        }
        serde_json::Value::Array(rows) => {
            summary_items.push(serde_json::json!({"type": "label", "key": "数据条数", "value": rows.len().to_string()}));
            data_items.push(json_array_item("返回列表", rows));
        }
        _ => {
            data_items.push(serde_json::json!({"type": "label", "key": "返回值", "value": scalar_json_display(&value)}));
        }
    }

    if data_items.is_empty() {
        data_items.push(serde_json::json!({"type": "info", "text": serde_json::to_string_pretty(&value).unwrap_or_default()}));
    }

    serde_json::json!({
        "name": "脚本执行结果",
        "status": status,
        "exit_code": exit_code,
        "elapsed_ms": elapsed_ms,
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "sections": [
            {
                "title": "执行摘要",
                "icon": "SUMMARY",
                "items": summary_items
            },
            {
                "title": "返回数据",
                "icon": "DATA",
                "items": data_items
            }
        ]
    })
}

fn scalar_json_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "-".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn json_complex_item(key: &str, value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(rows) => json_array_item(key, rows),
        _ => serde_json::json!({
            "type": "info",
            "text": serde_json::to_string_pretty(value).unwrap_or_default()
        }),
    }
}

fn json_array_item(title: &str, rows: &[serde_json::Value]) -> serde_json::Value {
    if rows.iter().all(|row| row.is_object()) {
        let mut headers = Vec::<String>::new();
        for row in rows {
            if let Some(map) = row.as_object() {
                for key in map.keys() {
                    if !headers.contains(key) {
                        headers.push(key.clone());
                    }
                }
            }
        }
        let table_rows: Vec<Vec<String>> = rows
            .iter()
            .filter_map(|row| row.as_object())
            .map(|map| {
                headers
                    .iter()
                    .map(|key| {
                        map.get(key)
                            .map(scalar_json_display)
                            .unwrap_or_else(|| "-".to_string())
                    })
                    .collect()
            })
            .collect();
        return serde_json::json!({
            "type": "table",
            "headers": headers,
            "rows": table_rows
        });
    }

    serde_json::json!({
        "type": "info",
        "text": format!("{}:\n{}", title, serde_json::to_string_pretty(rows).unwrap_or_default())
    })
}

fn try_parse_json_output(output: &str, exit_code: i32, elapsed_ms: u64) -> String {
    let cleaned = clean_terminal_output(output);
    let trimmed = cleaned.trim();

    if let Some(json) = parse_json_candidate(trimmed) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
            return json_value_to_result(value, exit_code, elapsed_ms).to_string();
        }
        return json;
    }

    for block in fenced_blocks(trimmed) {
        if let Some(json) = parse_json_candidate(&block) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                return json_value_to_result(value, exit_code, elapsed_ms).to_string();
            }
            return json;
        }
    }

    for line in trimmed.lines() {
        if let Some(json) = parse_json_candidate(line) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                return json_value_to_result(value, exit_code, elapsed_ms).to_string();
            }
            return json;
        }
    }

    let status = if exit_code == 0 { "ok" } else { "error" };
    let lines: Vec<&str> = trimmed.lines().collect();
    let line_count = lines.len();

    serde_json::json!({
        "name": "脚本执行结果",
        "status": status,
        "exit_code": exit_code,
        "elapsed_ms": elapsed_ms,
        "line_count": line_count,
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "sections": [
            {
                "title": "执行摘要",
                "icon": "SUMMARY",
                "items": [
                    {"type": "label", "key": "状态", "value": if exit_code == 0 { "成功" } else { "失败" }, "status": status},
                    {"type": "label", "key": "退出码", "value": exit_code.to_string()},
                    {"type": "label", "key": "耗时", "value": format!("{}ms", elapsed_ms)},
                    {"type": "label", "key": "输出行数", "value": line_count.to_string()}
                ]
            },
            {
                "title": "执行输出",
                "icon": "OUTPUT",
                "items": [
                    {"type": "info", "text": trimmed}
                ]
            }
        ]
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::try_parse_json_output;

    #[test]
    fn parses_markdown_fenced_json_output() {
        let out = "说明\n```json\n{\"status\":\"ok\",\"sections\":[]}\n```";
        let parsed = try_parse_json_output(out, 0, 12);
        let value: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert_eq!(value["status"], "ok");
        assert!(value["sections"].is_array());
    }

    #[test]
    fn extracts_json_from_noisy_output() {
        let out = "prefix {\"status\":\"warn\",\"items\":[1,2]} suffix";
        let parsed = try_parse_json_output(out, 0, 12);
        let value: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert!(value.get("sections").is_some());
        assert_eq!(value["status"], "warn");
    }

    #[test]
    fn wraps_json_arrays_as_renderable_result() {
        let out = r#"[{"name":"nginx","status":"running"},{"name":"redis","status":"running"}]"#;
        let parsed = try_parse_json_output(out, 0, 12);
        let value: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert!(value.get("sections").is_some());
        assert_eq!(value["sections"][1]["items"][0]["type"], "table");
    }
}

async fn handle_dashboard_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut update_interval = interval(Duration::from_secs(10));

    let msg = serde_json::json!({
        "type": "connected",
        "message": "仪表盘实时数据已连接"
    });
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                let config = state.config.clone();
                let (sys, script_count) = tokio::task::spawn_blocking(move || {
                    let sys = crate::dashboard::get_system_info();
                    let dirs = crate::config::all_script_dirs(&config);
                    let script_count = script::discover_scripts(&dirs).map(|s| s.len()).unwrap_or(0);
                    (sys, script_count)
                })
                .await
                .unwrap_or_else(|_| (crate::dashboard::get_system_info(), 0));
                crate::web::api::record_metric_sample(&state.db, &sys);
                let (total_exec, success, failure) = state.db.get_stats();

                let mut processes: Vec<serde_json::Value> = sys.top_processes.iter().map(|p| {
                    serde_json::json!({
                        "pid": p.pid,
                        "name": p.name,
                        "cpu_usage": p.cpu_usage,
                        "memory_bytes": p.memory_bytes,
                    })
                }).collect();

                processes.sort_by(|a, b| {
                    let a_cpu = a["cpu_usage"].as_f64().unwrap_or(0.0);
                    let b_cpu = b["cpu_usage"].as_f64().unwrap_or(0.0);
                    let a_mem = a["memory_bytes"].as_u64().unwrap_or(0);
                    let b_mem = b["memory_bytes"].as_u64().unwrap_or(0);
                    let score_a = a_cpu * 0.6 + (a_mem as f64 / 1024.0 / 1024.0) * 0.4;
                    let score_b = b_cpu * 0.6 + (b_mem as f64 / 1024.0 / 1024.0) * 0.4;
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });

                let data = serde_json::json!({
                    "type": "update",
                    "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    "system": {
                        "cpu_usage": sys.cpu_usage,
                        "memory_total": sys.memory_total,
                        "memory_used": sys.memory_used,
                        "memory_usage": sys.memory_usage,
                        "disk_total": sys.disk_total,
                        "disk_used": sys.disk_used,
                        "disk_usage": sys.disk_usage,
                        "load_avg": {
                            "one": sys.load_avg.one,
                            "five": sys.load_avg.five,
                            "fifteen": sys.load_avg.fifteen,
                        },
                        "cpu_count": sys.cpu_count,
                        "cpu_brand": sys.cpu_brand,
                        "hostname": sys.hostname,
                        "os": sys.os,
                        "kernel": sys.kernel,
                        "arch": sys.arch,
                        "uptime": sys.uptime,
                        "process_count": sys.process_count,
                        "networks": sys.networks.iter().map(|n| {
                            serde_json::json!({
                                "name": n.name,
                                "ip": n.ip,
                                "mac": n.mac,
                                "received_bytes": n.received_bytes,
                                "transmitted_bytes": n.transmitted_bytes,
                            })
                        }).collect::<Vec<_>>(),
                        "disks": sys.disks.iter().map(|d| {
                            serde_json::json!({
                                "name": d.name,
                                "mount_point": d.mount_point,
                                "total": d.total,
                                "used": d.used,
                                "usage": d.usage,
                                "fs_type": d.fs_type,
                            })
                        }).collect::<Vec<_>>(),
                        "top_processes": processes,
                    },
                    "stats": {
                        "total_scripts": script_count,
                        "total_executions": total_exec,
                        "success_count": success,
                        "failure_count": failure,
                    }
                });

                if let Ok(json) = serde_json::to_string(&data) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    _ => continue,
                }
            }
        }
    }
}
