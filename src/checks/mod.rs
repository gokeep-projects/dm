//! Rust-native 检查模块
//!
//! 所有检查项返回结构化 JSON，前端/CLI 负责渲染。
//! 插件（外部可执行文件）也返回相同格式的 JSON。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub version: String,
    pub timestamp: String,
    pub duration_ms: u64,
    pub status: CheckStatus,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Error,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Item {
    Label {
        key: String,
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    Bar {
        key: String,
        value: f64,
        max: f64,
        unit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    Sparkline {
        key: String,
        data: Vec<f64>,
        unit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    Info {
        text: String,
    },
    Warning {
        text: String,
    },
    Error {
        text: String,
    },
    Success {
        text: String,
    },
    Finding {
        rule_id: String,
        level: String,
        category: String,
        title: String,
        target: String,
        summary: String,
        evidence: Vec<String>,
        suggestion: String,
        commands: Vec<String>,
    },
    Divider,
}

pub mod business_check;
pub mod common;
pub mod container;
pub mod elasticsearch;
pub mod environment;
pub mod java_service;
pub mod keepalived_check;
pub mod middleware;
pub mod mysql_check;
pub mod network;
pub mod nginx_check;
pub mod redis_check;
pub mod resource;
pub mod schedule;
pub mod security;
pub mod service;
pub mod service_manage;
pub mod smart_check;
pub mod system;

pub fn run_check(id: &str) -> Option<CheckResult> {
    let start = std::time::Instant::now();
    run_check_without_enrich(id).map(|mut r| {
        if r.duration_ms == 0 {
            r.duration_ms = start.elapsed().as_millis() as u64;
        }
        if r.timestamp.is_empty() {
            r.timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        }
        crate::anomaly::enrich_check_result(&mut r);
        r
    })
}

pub fn run_check_without_enrich(id: &str) -> Option<CheckResult> {
    match id {
        "system" => Some(system::check()),
        "security" => Some(security::check()),
        "network" => Some(network::check()),
        "resource" => Some(resource::check()),
        "service" => Some(service::check()),
        "environment" => Some(environment::check()),
        "container" => Some(container::check()),
        "middleware" => Some(middleware::check()),
        "schedule" => Some(schedule::check()),
        "smart-check" => Some(smart_check::check()),
        "service-manage" => Some(service_manage::check()),
        "elasticsearch" => Some(elasticsearch::check()),
        "redis" => Some(redis_check::check()),
        "nginx" => Some(nginx_check::check()),
        "keepalived" => Some(keepalived_check::check()),
        "mysql" => Some(mysql_check::check()),
        "java-service" => Some(java_service::check()),
        "business-check" => Some(business_check::check()),
        _ => run_plugin(id),
    }
}

pub fn list_checks() -> Vec<CheckInfo> {
    let mut checks = vec![
        CheckInfo {
            id: "system".into(),
            name: "系统综合检查".into(),
            description: "主机信息/CPU/内存/磁盘/网络/进程".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "security".into(),
            name: "安全策略检查".into(),
            description: "SELinux/AppArmor/防火墙/内核加固".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "network".into(),
            name: "网络诊断".into(),
            description: "网卡/端口/SSL/防火墙".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "resource".into(),
            name: "硬件资源".into(),
            description: "CPU/内存/磁盘/Swap/IO".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "service".into(),
            name: "服务状态".into(),
            description: "分类/端口/异常/状态".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "environment".into(),
            name: "环境信息".into(),
            description: "系统/变量/软件/用户".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "container".into(),
            name: "容器状态".into(),
            description: "Docker容器/镜像/状态".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "middleware".into(),
            name: "中间件".into(),
            description: "Nginx/Redis/MySQL/Kafka/ES".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "elasticsearch".into(),
            name: "Elasticsearch 健康检查".into(),
            description: "健康/存储/索引/分片/任务/日志/备份/配置".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "redis".into(),
            name: "Redis 常规检查".into(),
            description: "状态/连接/异常日志/Key 分布/AOF 修复".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "nginx".into(),
            name: "Nginx 常规检查".into(),
            description: "端口/反向代理/安全配置/异常日志".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "keepalived".into(),
            name: "Keepalived 常规检查".into(),
            description: "节点/VIP/脚本/日志/配置".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "mysql".into(),
            name: "MySQL 常规检查".into(),
            description: "连接/数据库统计/异常日志/备份恢复".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "java-service".into(),
            name: "Java 服务常规检查".into(),
            description: "服务前缀/进程/线程/端口/异常日志/堆栈建议".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "business-check".into(),
            name: "业务综合检查".into(),
            description: "统一汇总系统、中间件、Java 服务、网络与安全异常".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "schedule".into(),
            name: "定时任务".into(),
            description: "crontab/系统cron/服务状态".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "smart-check".into(),
            name: "智能全量体检".into(),
            description: "智能综合检查：系统配置/服务/安全".into(),
            category: "常规检查".into(),
        },
        CheckInfo {
            id: "service-manage".into(),
            name: "服务管理".into(),
            description: "管理系统内所有服务：Java优先 > 中间件 > 系统进程".into(),
            category: "服务管理".into(),
        },
    ];
    checks.extend(discover_plugins());
    checks
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
}

fn plugins_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".dm").join("plugins")
}

pub fn discover_plugins() -> Vec<CheckInfo> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&dir).into_iter().flatten() {
        let Ok(e) = entry else { continue };
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if p.metadata()
                .ok()
                .map(|m| m.permissions().mode() & 0o111 == 0)
                .unwrap_or(true)
            {
                continue;
            }
        }
        let id = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let name = id.replace('-', " ").replace('_', " ");
        plugins.push(CheckInfo {
            id: id.clone(),
            name: name.clone(),
            description: "外部插件".to_string(),
            category: "插件".to_string(),
        });
    }
    plugins
}

pub fn run_plugin(id: &str) -> Option<CheckResult> {
    let dir = plugins_dir();
    let path = dir.join(id);
    if !path.exists() {
        return None;
    }

    let start = std::time::Instant::now();
    let output = std::process::Command::new(&path).output().ok()?;

    let elapsed = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    match serde_json::from_str::<CheckResult>(&stdout) {
        Ok(mut result) => {
            result.duration_ms = elapsed;
            result.timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            Some(result)
        }
        Err(_) => {
            let mut items = vec![];

            if exit_code != 0 {
                items.push(Item::Error {
                    text: format!("插件执行失败 (退出码: {})", exit_code),
                });
            }

            if !stdout.is_empty() {
                items.push(Item::Label {
                    key: "输出内容".to_string(),
                    value: stdout.chars().take(200).collect::<String>(),
                    status: None,
                });
            }

            if !stderr.is_empty() {
                items.push(Item::Warning {
                    text: format!("错误输出: {}", stderr.chars().take(200).collect::<String>()),
                });
            }

            if items.is_empty() {
                items.push(Item::Warning {
                    text: "插件没有返回任何输出".to_string(),
                });
            }

            items.push(Item::Divider);
            items.push(Item::Info {
                text: "提示: 插件应该返回 JSON 格式的检查结果".to_string(),
            });

            Some(CheckResult {
                id: id.to_string(),
                name: id.to_string(),
                description: "外部插件".to_string(),
                category: "插件".to_string(),
                version: "1.0.0".to_string(),
                timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                duration_ms: elapsed,
                status: if exit_code == 0 {
                    CheckStatus::Warn
                } else {
                    CheckStatus::Error
                },
                sections: vec![Section {
                    title: "插件执行结果".to_string(),
                    icon: Some("🔌".to_string()),
                    description: None,
                    items,
                }],
            })
        }
    }
}
