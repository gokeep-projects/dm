use super::common::*;
use super::*;
use std::path::PathBuf;

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("mysql");
    let (host, port) = cfg.address("127.0.0.1", "3306");
    let config_path = configured_or_first(
        &cfg.config_path,
        &[
            "/etc/mysql/my.cnf",
            "/etc/my.cnf",
            "/etc/mysql/mysql.conf.d/mysqld.cnf",
        ],
    );
    let log_path = configured_or_first(
        &cfg.log_path,
        &["/var/log/mysql/error.log", "/var/log/mysqld.log"],
    );
    let data_path = configured_or_first(&cfg.data_path, &["/var/lib/mysql"]);
    let mysql = find_command("mysql").unwrap_or_else(|| "mysql".to_string());
    let auth = mysql_auth(&cfg);
    let base = format!(
        "{} -h {} -P {} {}",
        shell_escape(&mysql),
        shell_escape(&host),
        shell_escape(&port),
        auth
    );
    let ping = shell_output(&format!(
        "{} -e 'SELECT VERSION() as version' 2>/dev/null",
        base
    ));
    let status = shell_output(&format!("{} -e 'SHOW GLOBAL STATUS' 2>/dev/null", base));
    let variables = shell_output(&format!("{} -e 'SHOW VARIABLES' 2>/dev/null", base));
    let dbs = shell_output(&format!("{} -e 'SELECT table_schema,COUNT(*) tables,ROUND(SUM(data_length+index_length)/1024/1024,2) mb FROM information_schema.tables GROUP BY table_schema ORDER BY mb DESC' 2>/dev/null", base));
    let backup = shell_output(&format!("{} -e \"SHOW VARIABLES WHERE Variable_name IN ('log_bin','binlog_format','expire_logs_days','binlog_expire_logs_seconds','datadir','secure_file_priv')\" 2>/dev/null", base));
    let connected = ping
        .as_deref()
        .map(|v| v.to_lowercase().contains("version"))
        .unwrap_or(false);

    let sections = vec![
        Section {
            title: "连接信息".to_string(),
            icon: Some("MYSQL".to_string()),
            description: Some(
                "连接配置持久化在数据库；未配置时默认探测 127.0.0.1:3306".to_string(),
            ),
            items: vec![
                label(
                    "地址",
                    format!("{}:{}", host, port),
                    Some(if connected { "ok" } else { "warn" }),
                ),
                label(
                    "用户",
                    if cfg.username.is_empty() {
                        "未配置"
                    } else {
                        &cfg.username
                    },
                    None,
                ),
                label(
                    "连接状态",
                    if connected {
                        "可连接"
                    } else {
                        "不可连接或未配置认证"
                    },
                    Some(if connected { "ok" } else { "warn" }),
                ),
                label("配置路径", path_text(config_path.as_ref()), None),
                label("数据路径", path_text(data_path.as_ref()), None),
                label("日志路径", path_text(log_path.as_ref()), None),
            ],
        },
        text_section("版本探测", ping.as_deref(), "未获取到版本"),
        kv_filter_section(
            "关键状态",
            status.as_deref(),
            &[
                "Threads_connected",
                "Threads_running",
                "Connections",
                "Aborted_connects",
                "Slow_queries",
                "Innodb_buffer_pool_reads",
                "Innodb_row_lock_waits",
                "Uptime",
            ],
        ),
        kv_filter_section(
            "关键配置",
            variables.as_deref(),
            &[
                "version",
                "datadir",
                "log_error",
                "slow_query_log",
                "long_query_time",
                "max_connections",
                "innodb_buffer_pool_size",
                "character_set_server",
                "time_zone",
            ],
        ),
        text_section("数据库统计", dbs.as_deref(), "未获取到数据库统计"),
        text_section(
            "备份恢复信息",
            backup.as_deref(),
            "未获取到 binlog/备份恢复相关变量",
        ),
        config_preview_section("配置文件", config_path),
        log_section("关键异常日志", log_path, 100),
        table_section(
            "进程信息",
            vec!["PID", "PPID", "用户", "状态", "CPU", "内存", "命令"],
            process_rows(&["mysqld", "mariadbd"]),
            "未发现 MySQL/MariaDB 进程",
        ),
        table_section(
            "监听端口",
            vec!["协议", "本地地址", "对端", "进程"],
            listen_rows(&["mysqld", "mariadbd"], &[&port]),
            "未发现 MySQL 监听端口",
        ),
    ];
    CheckResult {
        id: "mysql".to_string(),
        name: "MySQL 常规检查".to_string(),
        description: "连接/异常日志/数据库统计/关键配置/备份恢复/路径".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(connected || !process_rows(&["mysqld", "mariadbd"]).is_empty()),
        sections,
    }
}

fn mysql_auth(cfg: &EndpointConfig) -> String {
    let mut args = String::new();
    if !cfg.username.trim().is_empty() {
        args.push_str(&format!("-u {}", shell_escape(&cfg.username)));
    }
    if !cfg.password.trim().is_empty() {
        args.push_str(&format!(" -p{}", shell_escape(&cfg.password)));
    }
    args
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
fn kv_filter_section(title: &str, text: Option<&str>, keys: &[&str]) -> Section {
    let body = text.unwrap_or("");
    let mut rows = Vec::new();
    for key in keys {
        if let Some(line) = body
            .lines()
            .find(|line| line.split_whitespace().next() == Some(*key))
        {
            let value = line
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            rows.push(vec![key.to_string(), value]);
        }
    }
    table_section(title, vec!["指标", "值"], rows, "未获取到数据")
}
