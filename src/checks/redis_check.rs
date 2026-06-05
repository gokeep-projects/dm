use super::common::*;
use super::*;
use std::path::PathBuf;

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("redis");
    let (host, port) = cfg.address("127.0.0.1", "6379");
    let config_path = configured_or_first(
        &cfg.config_path,
        &[
            "/etc/redis/redis.conf",
            "/etc/redis.conf",
            "/usr/local/etc/redis.conf",
        ],
    );
    let log_path = configured_or_first(
        &cfg.log_path,
        &[
            "/var/log/redis/redis-server.log",
            "/var/log/redis/redis.log",
        ],
    );
    let data_path = configured_or_first(&cfg.data_path, &["/var/lib/redis", "/data/redis"]);
    let cli = find_command("redis-cli").unwrap_or_else(|| "redis-cli".to_string());
    let auth = if cfg.password.trim().is_empty() {
        String::new()
    } else {
        format!(" -a {}", shell_escape(&cfg.password))
    };
    let base_cmd = format!(
        "{} -h {} -p {}{} --no-auth-warning",
        shell_escape(&cli),
        shell_escape(&host),
        shell_escape(&port),
        auth
    );
    let ping = shell_output(&format!("{} ping 2>/dev/null", base_cmd));
    let info = shell_output(&format!("{} info 2>/dev/null", base_cmd));
    let cluster = shell_output(&format!("{} cluster info 2>/dev/null", base_cmd));
    let keyspace = info_section(&info, "Keyspace");
    let connected = ping.as_deref().map(|v| v.contains("PONG")).unwrap_or(false);

    let mut sections = Vec::new();
    sections.push(Section {
        title: "连接信息".to_string(),
        icon: Some("REDIS".to_string()),
        description: Some("连接配置持久化在数据库；未配置时默认探测 127.0.0.1:6379".to_string()),
        items: vec![
            label(
                "地址",
                format!("{}:{}", host, port),
                Some(if connected { "ok" } else { "warn" }),
            ),
            label(
                "认证",
                if cfg.password.is_empty() {
                    "未配置密码"
                } else {
                    "已配置密码"
                },
                None,
            ),
            label(
                "PING",
                ping.unwrap_or_else(|| "无响应".to_string()),
                Some(if connected { "ok" } else { "warn" }),
            ),
            label("配置路径", path_text(config_path.as_ref()), None),
            label("数据路径", path_text(data_path.as_ref()), None),
            label("日志路径", path_text(log_path.as_ref()), None),
            label("客户端", cli, None),
        ],
    });
    sections.push(info_table(
        "运行状态",
        &info,
        &[
            "redis_version",
            "redis_mode",
            "role",
            "uptime_in_seconds",
            "connected_clients",
            "blocked_clients",
            "used_memory_human",
            "maxmemory_human",
            "mem_fragmentation_ratio",
            "loading",
            "rdb_last_bgsave_status",
            "aof_enabled",
            "aof_last_bgrewrite_status",
            "aof_last_write_status",
        ],
    ));
    sections.push(text_section(
        "集群信息",
        cluster.as_deref(),
        "未启用集群或无法获取 cluster info",
    ));
    sections.push(text_section(
        "Key 分布与命名空间",
        keyspace.as_deref(),
        "未获取到 keyspace 信息",
    ));
    sections.push(Section {
        title: "AOF 快速修复".to_string(),
        icon: Some("FIX".to_string()),
        description: Some(
            "页面/CLI 提供修复建议命令；真正执行前请先备份 appendonly 文件".to_string(),
        ),
        items: vec![
            Item::Warning {
                text: "AOF 损坏时先停止 Redis 并备份 appendonly.aof，再执行 redis-check-aof --fix"
                    .to_string(),
            },
            label(
                "建议命令 1",
                "systemctl stop redis || systemctl stop redis-server",
                None,
            ),
            label(
                "建议命令 2",
                "cp appendonly.aof appendonly.aof.bak.$(date +%F-%H%M%S)",
                None,
            ),
            label("建议命令 3", "redis-check-aof --fix appendonly.aof", None),
            label(
                "建议命令 4",
                "systemctl start redis || systemctl start redis-server",
                None,
            ),
        ],
    });
    sections.push(config_preview_section("配置信息", config_path));
    sections.push(log_section("异常日志", log_path, 100));
    sections.push(table_section(
        "进程信息",
        vec!["PID", "PPID", "用户", "状态", "CPU", "内存", "命令"],
        process_rows(&["redis-server"]),
        "未发现 redis-server 进程",
    ));
    sections.push(table_section(
        "监听端口",
        vec!["协议", "本地地址", "对端", "进程"],
        listen_rows(&["redis"], &[&port]),
        "未发现 Redis 监听端口",
    ));

    CheckResult {
        id: "redis".to_string(),
        name: "Redis 常规检查".to_string(),
        description: "状态/连接/日志/Key分布/AOF修复/配置/路径".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(connected),
        sections,
    }
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
fn info_section(info: &Option<String>, section: &str) -> Option<String> {
    let mut capture = false;
    let mut out = Vec::new();
    for line in info.as_deref().unwrap_or("").lines() {
        if line.trim() == format!("# {}", section) {
            capture = true;
            continue;
        }
        if capture && line.starts_with('#') {
            break;
        }
        if capture && !line.trim().is_empty() {
            out.push(line.to_string());
        }
    }
    (!out.is_empty()).then(|| out.join("\n"))
}
fn info_table(title: &str, info: &Option<String>, keys: &[&str]) -> Section {
    let text = info.as_deref().unwrap_or("");
    let mut rows = Vec::new();
    for key in keys {
        let value = text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{}:", key)))
            .unwrap_or("-");
        rows.push(vec![key.to_string(), value.to_string()]);
    }
    table_section(title, vec!["指标", "值"], rows, "未获取到 INFO")
}
fn text_section(title: &str, text: Option<&str>, empty: &str) -> Section {
    let rows = text
        .unwrap_or("")
        .lines()
        .take(200)
        .enumerate()
        .map(|(i, v)| vec![(i + 1).to_string(), truncate(v, 260)])
        .collect();
    table_section(title, vec!["行", "内容"], rows, empty)
}
