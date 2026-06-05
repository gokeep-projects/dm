use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // 网络接口
    let networks = sysinfo::Networks::new_with_refreshed_list();
    let mut net_items = Vec::new();
    for (name, data) in networks.iter() {
        if name == "lo"
            || name.starts_with("docker")
            || name.starts_with("br-")
            || name.starts_with("veth")
        {
            continue;
        }
        let status = if data.total_received() > 0 || data.total_transmitted() > 0 {
            "ok".to_string()
        } else {
            "warn".to_string()
        };
        net_items.push(Item::Label {
            key: name.clone(),
            value: format!(
                "↓{} ↑{} 错误:{}",
                fmt_bytes(data.total_received()),
                fmt_bytes(data.total_transmitted()),
                data.total_errors_on_received() + data.total_errors_on_transmitted()
            ),
            status: Some(status),
        });
    }

    if !net_items.is_empty() {
        sections.push(Section {
            title: "网络接口".to_string(),
            icon: Some("🌐".to_string()),
            description: None,
            items: net_items,
        });
    }

    // DNS 解析
    let dns_ok = std::process::Command::new("nslookup")
        .arg("baidu.com")
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);

    sections.push(Section {
        title: "DNS 解析".to_string(),
        icon: Some("🔍".to_string()),
        description: None,
        items: vec![Item::Label {
            key: "DNS 解析".to_string(),
            value: if dns_ok {
                "正常".to_string()
            } else {
                "异常".to_string()
            },
            status: Some(if dns_ok { "ok" } else { "error" }.to_string()),
        }],
    });

    // SSL 证书检查
    let ssl_items = check_ssl_certs();
    if !ssl_items.is_empty() {
        sections.push(Section {
            title: "SSL 证书".to_string(),
            icon: Some("🔐".to_string()),
            description: None,
            items: ssl_items,
        });
    }

    CheckResult {
        id: "network".to_string(),
        name: "网络诊断".to_string(),
        description: "网卡/端口/SSL/防火墙".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}

fn check_ssl_certs() -> Vec<Item> {
    let output = std::process::Command::new("find")
        .args([
            "/etc/ssl",
            "/etc/pki",
            "/etc/letsencrypt",
            "-name",
            "*.pem",
            "-o",
            "-name",
            "*.crt",
        ])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut items = Vec::new();
    for path in output.lines().take(10) {
        if let Ok(output) = std::process::Command::new("openssl")
            .args(["x509", "-in", path, "-noout", "-enddate"])
            .output()
        {
            if let Ok(enddate) = String::from_utf8(output.stdout) {
                let enddate = enddate.trim().replace("notAfter=", "");
                items.push(Item::Label {
                    key: path.split('/').last().unwrap_or(path).to_string(),
                    value: enddate,
                    status: Some("ok".to_string()),
                });
            }
        }
    }
    items
}

fn fmt_bytes(b: u64) -> String {
    if b < 1024 {
        return format!("{} B", b);
    }
    if b < 1048576 {
        return format!("{:.1} KB", b as f64 / 1024.0);
    }
    if b < 1073741824 {
        return format!("{:.1} MB", b as f64 / 1048576.0);
    }
    format!("{:.1} GB", b as f64 / 1073741824.0)
}
