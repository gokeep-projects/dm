use super::common::*;
use super::*;
use std::path::PathBuf;

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("keepalived");
    let config_path = configured_or_first(&cfg.config_path, &["/etc/keepalived/keepalived.conf"]);
    let log_path = configured_or_first(&cfg.log_path, &["/var/log/syslog", "/var/log/messages"]);
    let running = !process_rows(&["keepalived"]).is_empty();
    let config = config_path
        .as_ref()
        .and_then(|p| read_file_limited(p, 128 * 1024))
        .unwrap_or_default();
    let nodes = extract_keepalived(
        &config,
        &[
            "virtual_router_id",
            "state",
            "priority",
            "interface",
            "unicast_src_ip",
            "unicast_peer",
        ],
    );
    let vips = extract_blocks(&config, "virtual_ipaddress");
    let scripts = extract_blocks(&config, "vrrp_script");

    let sections = vec![
        Section {
            title: "运行状态".to_string(),
            icon: Some("KA".to_string()),
            description: Some("Keepalived VRRP 节点、VIP、脚本和日志分析".to_string()),
            items: vec![
                label(
                    "运行状态",
                    if running {
                        "运行中"
                    } else {
                        "未发现运行进程"
                    },
                    Some(if running { "ok" } else { "warn" }),
                ),
                label("配置路径", path_text(config_path.as_ref()), None),
                label("日志路径", path_text(log_path.as_ref()), None),
            ],
        },
        table_section(
            "节点信息",
            vec!["字段", "值"],
            nodes,
            "未解析到 VRRP 节点字段",
        ),
        table_section(
            "虚拟 IP",
            vec!["序号", "VIP/配置"],
            vips,
            "未解析到 virtual_ipaddress",
        ),
        table_section(
            "执行脚本",
            vec!["序号", "脚本配置"],
            scripts,
            "未解析到 vrrp_script",
        ),
        config_preview_section("配置信息", config_path),
        log_section("异常日志", log_path, 100),
        table_section(
            "进程信息",
            vec!["PID", "PPID", "用户", "状态", "CPU", "内存", "命令"],
            process_rows(&["keepalived"]),
            "未发现 Keepalived 进程",
        ),
    ];
    CheckResult {
        id: "keepalived".to_string(),
        name: "Keepalived 常规检查".to_string(),
        description: "节点/地址/VIP/脚本/日志/配置".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(running),
        sections,
    }
}

fn extract_keepalived(config: &str, keys: &[&str]) -> Vec<Vec<String>> {
    config
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            keys.iter()
                .find(|key| trimmed.starts_with(**key))
                .map(|key| vec![key.to_string(), truncate(trimmed, 220)])
        })
        .collect()
}
fn extract_blocks(config: &str, name: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut capture = false;
    let mut current = Vec::new();
    let mut depth = 0;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(name) {
            capture = true;
            depth = 0;
            current.clear();
        }
        if capture {
            if trimmed.contains('{') {
                depth += 1;
            }
            if trimmed.contains('}') {
                depth -= 1;
            }
            if !trimmed.is_empty() {
                current.push(trimmed.to_string());
            }
            if depth <= 0 && trimmed.contains('}') {
                rows.push(vec![
                    (rows.len() + 1).to_string(),
                    truncate(&current.join(" | "), 260),
                ]);
                capture = false;
            }
        }
    }
    rows
}
fn configured_or_first(value: &str, defaults: &[&str]) -> Option<PathBuf> {
    if !value.trim().is_empty() {
        Some(PathBuf::from(value))
    } else {
        first_existing(defaults)
    }
}
fn path_text(path: Option<&PathBuf>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "未推断到".to_string())
}
