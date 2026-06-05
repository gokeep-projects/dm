use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // 系统服务
    let services = vec![
        ("sshd", "SSH 服务"),
        ("docker", "Docker 服务"),
        ("nginx", "Nginx 服务"),
        ("redis", "Redis 服务"),
        ("mysql", "MySQL 服务"),
        ("postgresql", "PostgreSQL 服务"),
        ("caddy", "Caddy 服务"),
    ];

    let mut service_items = Vec::new();
    for (name, desc) in services {
        let status = check_service_status(name);
        if status != "未安装" {
            service_items.push(Item::Label {
                key: desc.to_string(),
                value: status.clone(),
                status: Some(
                    if status.contains("运行中") || status.contains("active") {
                        "ok"
                    } else {
                        "warn"
                    }
                    .to_string(),
                ),
            });
        }
    }

    if service_items.is_empty() {
        service_items.push(Item::Info {
            text: "未检测到常用服务".to_string(),
        });
    }

    sections.push(Section {
        title: "系统服务".to_string(),
        icon: Some("🔧".to_string()),
        description: None,
        items: service_items,
    });

    CheckResult {
        id: "service".to_string(),
        name: "服务状态".to_string(),
        description: "分类/端口/异常/状态".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}

fn check_service_status(name: &str) -> String {
    // Try systemctl first
    if let Ok(output) = std::process::Command::new("systemctl")
        .args(["is-active", name])
        .output()
    {
        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if status == "active" {
            return "运行中".to_string();
        } else if status == "inactive" {
            return "已停止".to_string();
        }
    }

    // Try service command
    if let Ok(output) = std::process::Command::new("service")
        .args([name, "status"])
        .output()
    {
        let status = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if status.contains("running") || status.contains("active") {
            return "运行中".to_string();
        } else if status.contains("stopped") || status.contains("inactive") {
            return "已停止".to_string();
        }
    }

    "未安装".to_string()
}
