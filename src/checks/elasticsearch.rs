use super::common::*;
use super::*;
use serde_json::Value;
use std::path::PathBuf;

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("elasticsearch");
    let base = cfg.base_url("http", "127.0.0.1", "9200");
    let config_path = configured_or_first(
        &cfg.config_path,
        &[
            "/etc/elasticsearch/elasticsearch.yml",
            "/usr/local/etc/elasticsearch/elasticsearch.yml",
            "/opt/elasticsearch/config/elasticsearch.yml",
        ],
    );
    let log_path = configured_or_first(
        &cfg.log_path,
        &[
            "/var/log/elasticsearch/elasticsearch.log",
            "/opt/elasticsearch/logs/elasticsearch.log",
        ],
    );
    let data_path = configured_or_first(
        &cfg.data_path,
        &["/var/lib/elasticsearch", "/opt/elasticsearch/data"],
    );
    let program_path = if cfg.program_path.trim().is_empty() {
        find_command("elasticsearch")
    } else {
        Some(cfg.program_path.clone())
    };

    let root = curl_json(&base, "", &cfg);
    let health = curl_json(&base, "/_cluster/health?pretty", &cfg);
    let nodes = curl_json(&base, "/_nodes/process,jvm,settings,fs,http?pretty", &cfg);
    let indices = curl_text(&base, "/_cat/indices?v&bytes=mb&s=store.size:desc", &cfg);
    let shards = curl_text(&base, "/_cat/shards?v&s=state,index", &cfg);
    let tasks = curl_json(&base, "/_tasks?detailed=true&pretty", &cfg);
    let snapshots = curl_json(&base, "/_snapshot?pretty", &cfg);

    let connected = root.is_some() || health.is_some();
    let mut sections = Vec::new();
    sections.push(Section {
        title: "连接信息".to_string(),
        icon: Some("ES".to_string()),
        description: Some(
            "连接信息会从数据库配置读取；未配置时默认探测 http://127.0.0.1:9200".to_string(),
        ),
        items: vec![
            label(
                "访问地址",
                base.clone(),
                Some(if connected { "ok" } else { "warn" }),
            ),
            label("认证用户", mask_empty(&cfg.username), None),
            label(
                "连接状态",
                if connected {
                    "可连接"
                } else {
                    "不可连接或未启动"
                },
                Some(if connected { "ok" } else { "warn" }),
            ),
            label(
                "程序路径",
                program_path.unwrap_or_else(|| "未推断到".to_string()),
                None,
            ),
            label("配置路径", path_text(config_path.as_ref()), None),
            label(
                "数据路径",
                cfg.data_path
                    .clone()
                    .if_empty(path_text(data_path.as_ref())),
                None,
            ),
            label("日志路径", path_text(log_path.as_ref()), None),
        ],
    });

    sections.push(Section {
        title: "集群健康".to_string(),
        icon: Some("HEALTH".to_string()),
        description: Some("ES 7.x 优先；旧版本接口兼容 _cluster/health/_cat".to_string()),
        items: es_health_items(health.as_ref()),
    });
    sections.push(json_summary_section(
        "节点与存储",
        nodes.as_ref(),
        &["nodes"],
        80,
    ));
    sections.push(text_table_section(
        "索引状态",
        indices.as_deref(),
        "未获取到索引列表",
    ));
    sections.push(text_table_section(
        "分片状态",
        shards.as_deref(),
        "未获取到分片信息",
    ));
    sections.push(json_summary_section(
        "当前任务",
        tasks.as_ref(),
        &["nodes"],
        80,
    ));
    sections.push(json_summary_section(
        "备份还原",
        snapshots.as_ref(),
        &[],
        80,
    ));
    sections.push(config_preview_section("配置信息", config_path));
    sections.push(log_section("日常异常日志", log_path, 100));
    sections.push(table_section(
        "进程与端口",
        vec!["PID", "PPID", "用户", "状态", "CPU", "内存", "命令"],
        process_rows(&["elasticsearch", "org.elasticsearch.bootstrap"]),
        "未发现 Elasticsearch 进程",
    ));
    sections.push(table_section(
        "监听端口",
        vec!["协议", "本地地址", "对端", "进程"],
        listen_rows(&["elasticsearch"], &["9200", "9300"]),
        "未发现 9200/9300 监听端口",
    ));

    CheckResult {
        id: "elasticsearch".to_string(),
        name: "Elasticsearch 健康检查".to_string(),
        description: "健康状态/存储/索引/分片/任务/日志/备份/配置/路径".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(connected),
        sections,
    }
}

fn es_health_items(health: Option<&Value>) -> Vec<Item> {
    let Some(h) = health else {
        return vec![Item::Warning {
            text: "无法获取 _cluster/health，请检查地址、认证、网络和 ES 服务状态".to_string(),
        }];
    };
    let status = h["status"].as_str().unwrap_or("unknown");
    let level = match status {
        "green" => "ok",
        "yellow" => "warn",
        "red" => "error",
        _ => "warn",
    };
    vec![
        label("集群状态", status, Some(level)),
        label("集群名称", strv(&h["cluster_name"]), None),
        label("节点数", strv(&h["number_of_nodes"]), None),
        label("数据节点", strv(&h["number_of_data_nodes"]), None),
        label("活跃主分片", strv(&h["active_primary_shards"]), None),
        label("活跃分片", strv(&h["active_shards"]), None),
        label(
            "未分配分片",
            strv(&h["unassigned_shards"]),
            Some(if h["unassigned_shards"].as_i64().unwrap_or(0) > 0 {
                "warn"
            } else {
                "ok"
            }),
        ),
        label("初始化分片", strv(&h["initializing_shards"]), None),
        label("迁移分片", strv(&h["relocating_shards"]), None),
        label("延迟未分配", strv(&h["delayed_unassigned_shards"]), None),
    ]
}

pub fn curl_json(base: &str, path: &str, cfg: &EndpointConfig) -> Option<Value> {
    let text = curl_text(base, path, cfg)?;
    serde_json::from_str(&text).ok()
}

pub fn curl_text(base: &str, path: &str, cfg: &EndpointConfig) -> Option<String> {
    let mut cmd = format!(
        "curl -fsS --max-time 5 {}",
        shell_escape(&format!("{}{}", base.trim_end_matches('/'), path))
    );
    if !cfg.username.trim().is_empty() {
        cmd = format!(
            "curl -fsS --max-time 5 -u {}:{} {}",
            shell_escape(&cfg.username),
            shell_escape(&cfg.password),
            shell_escape(&format!("{}{}", base.trim_end_matches('/'), path))
        );
    }
    shell_output(&cmd).filter(|s| !s.trim().is_empty())
}

fn configured_or_first(value: &str, defaults: &[&str]) -> Option<PathBuf> {
    if !value.trim().is_empty() {
        return Some(PathBuf::from(value));
    }
    first_existing(defaults)
}

fn json_summary_section(
    title: &str,
    value: Option<&Value>,
    keys: &[&str],
    limit: usize,
) -> Section {
    let mut rows = Vec::new();
    if let Some(v) = value {
        if keys.is_empty() {
            rows.push(vec![
                "json".to_string(),
                truncate(&serde_json::to_string_pretty(v).unwrap_or_default(), 600),
            ]);
        } else {
            for key in keys {
                rows.push(vec![
                    key.to_string(),
                    truncate(
                        &serde_json::to_string_pretty(&v[*key]).unwrap_or_default(),
                        600,
                    ),
                ]);
            }
        }
    }
    if rows.len() > limit {
        rows.truncate(limit);
    }
    table_section(title, vec!["字段", "内容"], rows, "未获取到数据")
}

fn text_table_section(title: &str, text: Option<&str>, empty: &str) -> Section {
    let rows = text
        .unwrap_or("")
        .lines()
        .take(200)
        .enumerate()
        .map(|(i, line)| vec![(i + 1).to_string(), truncate(line, 260)])
        .collect();
    table_section(title, vec!["行", "内容"], rows, empty)
}

fn path_text(path: Option<&PathBuf>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "未推断到".to_string())
}

fn mask_empty(value: &str) -> String {
    if value.trim().is_empty() {
        "未配置".to_string()
    } else {
        value.to_string()
    }
}

trait EmptyString {
    fn if_empty(self, alt: String) -> String;
}

impl EmptyString for String {
    fn if_empty(self, alt: String) -> String {
        if self.trim().is_empty() {
            alt
        } else {
            self
        }
    }
}

fn strv(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.to_string()
    } else {
        v.to_string()
    }
}
