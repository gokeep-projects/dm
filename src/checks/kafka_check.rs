use super::common::*;
use super::*;
use std::path::PathBuf;

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("kafka");
    let process_rows = process_rows(&["kafka.Kafka", "kafka", "server.properties"]);
    let running = !process_rows.is_empty();
    let installed = find_command("kafka-topics.sh").is_some()
        || find_command("kafka-server-start.sh").is_some()
        || running;
    let available = installed || running;
    let config_path = configured_or_first(
        &cfg.config_path,
        &[
            "/etc/kafka/server.properties",
            "/opt/kafka/config/server.properties",
            "/usr/local/kafka/config/server.properties",
        ],
    );
    let log_path = configured_or_first(
        &cfg.log_path,
        &[
            "/var/log/kafka/server.log",
            "/opt/kafka/logs/server.log",
            "/usr/local/kafka/logs/server.log",
        ],
    );
    let data_path = configured_or_first(
        &cfg.data_path,
        &["/var/lib/kafka", "/tmp/kafka-logs", "/data/kafka"],
    );
    let cli = find_command("kafka-topics.sh").unwrap_or_else(|| "kafka-topics.sh".to_string());
    let bootstrap = kafka_bootstrap(&cfg);
    let topics = if available {
        shell_output(&format!(
            "{} --bootstrap-server {} --list 2>/dev/null | head -100",
            shell_escape(&cli),
            shell_escape(&bootstrap)
        ))
    } else {
        None
    };

    let sections = vec![
        Section {
            title: "运行与连接".to_string(),
            icon: Some("KAFKA".to_string()),
            description: Some(
                "基于进程、常见端口、配置路径和 kafka-topics CLI 采集运行状态".to_string(),
            ),
            items: vec![
                label(
                    "运行状态",
                    if running {
                        "运行中"
                    } else if installed {
                        "已安装但未发现运行进程"
                    } else {
                        "未安装"
                    },
                    Some(if running { "ok" } else { "warn" }),
                ),
                label("Bootstrap", bootstrap, None),
                label(
                    "CLI",
                    if installed {
                        cli
                    } else {
                        "未安装".to_string()
                    },
                    None,
                ),
                label(
                    "配置路径",
                    path_text_if_available(config_path.as_ref(), available),
                    None,
                ),
                label(
                    "数据路径",
                    path_text_if_available(data_path.as_ref(), available),
                    None,
                ),
                label(
                    "日志路径",
                    path_text_if_available(log_path.as_ref(), available),
                    None,
                ),
            ],
        },
        text_section("Topic 列表", topics.as_deref(), "未获取到 Topic 列表"),
        if available {
            config_preview_section("配置文件", config_path)
        } else {
            unavailable_config_section("配置文件", "Kafka")
        },
        if available {
            log_section("关键异常日志", log_path, 120)
        } else {
            unavailable_log_section("关键异常日志", "Kafka")
        },
        table_section(
            "进程信息",
            vec!["PID", "PPID", "用户", "状态", "CPU", "内存", "命令"],
            process_rows,
            "未发现 Kafka 进程",
        ),
        table_section(
            "监听端口",
            vec!["协议", "本地地址", "对端", "进程"],
            listen_rows(&["kafka"], &["9092", "9093"]),
            "未发现 Kafka 监听端口",
        ),
    ];

    CheckResult {
        id: "kafka".to_string(),
        name: "Kafka 常规检查".to_string(),
        description: "进程/端口/Topic/配置/日志/数据路径".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(running),
        sections,
    }
}

fn kafka_bootstrap(cfg: &EndpointConfig) -> String {
    if !cfg.url.trim().is_empty() {
        return cfg.url.trim().to_string();
    }
    let (host, port) = cfg.address("127.0.0.1", "9092");
    format!("{}:{}", host, port)
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

fn path_text_if_available(path: Option<&PathBuf>, available: bool) -> String {
    if available {
        path_text(path)
    } else {
        "未检测到程序，跳过路径推断".to_string()
    }
}

fn text_section(title: &str, text: Option<&str>, empty: &str) -> Section {
    let rows = text
        .unwrap_or("")
        .lines()
        .take(200)
        .enumerate()
        .map(|(i, v)| vec![(i + 1).to_string(), truncate(v, 260)])
        .collect();
    table_section(title, vec!["序号", "内容"], rows, empty)
}
