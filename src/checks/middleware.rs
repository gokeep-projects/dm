use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // 中间件检查
    let middlewares = vec![
        ("nginx", "Nginx", vec!["nginx", "-v"]),
        ("redis-server", "Redis", vec!["redis-server", "--version"]),
        ("mysqld", "MySQL", vec!["mysql", "--version"]),
        ("kafka", "Kafka", vec!["kafka-topics.sh", "--version"]),
        (
            "elasticsearch",
            "Elasticsearch",
            vec!["elasticsearch", "--version"],
        ),
    ];

    let mut items = Vec::new();
    for (cmd, name, args) in middlewares {
        let status = check_middleware(cmd, &args);
        if status != "未安装" {
            items.push(Item::Label {
                key: name.to_string(),
                value: status.clone(),
                status: Some(
                    if status.contains("运行中") {
                        "ok"
                    } else {
                        "warn"
                    }
                    .to_string(),
                ),
            });
        }
    }

    if items.is_empty() {
        items.push(Item::Info {
            text: "未检测到常用中间件".to_string(),
        });
    }

    sections.push(Section {
        title: "中间件".to_string(),
        icon: Some("🔧".to_string()),
        description: None,
        items,
    });

    CheckResult {
        id: "middleware".to_string(),
        name: "中间件检查".to_string(),
        description: "Nginx/Redis/MySQL/Kafka/ES".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}

fn check_middleware(cmd: &str, args: &[&str]) -> String {
    if let Ok(output) = std::process::Command::new(cmd).args(args).output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            return format!("已安装 {}", version);
        }
    }
    "未安装".to_string()
}
