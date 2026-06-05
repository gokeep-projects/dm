use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // SELinux
    let selinux_status = std::fs::read_to_string("/etc/selinux/config")
        .ok()
        .and_then(|c| {
            c.lines()
                .find(|l| l.starts_with("SELINUX="))
                .map(|l| l.split('=').nth(1).unwrap_or("unknown").trim().to_string())
        })
        .unwrap_or_else(|| "未安装".to_string());

    sections.push(Section {
        title: "SELinux".to_string(),
        icon: Some("🛡️".to_string()),
        description: None,
        items: vec![Item::Label {
            key: "状态".to_string(),
            value: selinux_status.clone(),
            status: Some(
                if selinux_status == "enforcing" {
                    "ok"
                } else if selinux_status == "disabled" {
                    "warn"
                } else {
                    "info"
                }
                .to_string(),
            ),
        }],
    });

    // AppArmor
    let apparmor_status = std::process::Command::new("aa-status")
        .arg("--enabled")
        .output()
        .ok()
        .map(|o| {
            if o.status.success() {
                "已启用".to_string()
            } else {
                "未启用".to_string()
            }
        })
        .unwrap_or_else(|| "未安装".to_string());

    sections.push(Section {
        title: "AppArmor".to_string(),
        icon: Some("🔒".to_string()),
        description: None,
        items: vec![Item::Label {
            key: "状态".to_string(),
            value: apparmor_status,
            status: Some("info".to_string()),
        }],
    });

    // 防火墙
    let fw_items = vec![
        check_firewall_tool("iptables", &["-L", "-n"]),
        check_firewall_tool("nft", &["list", "ruleset"]),
        check_firewall_tool("ufw", &["status"]),
        check_firewall_tool("firewall-cmd", &["--state"]),
    ];

    let active_fw: Vec<Item> = fw_items.into_iter().filter_map(|x| x).collect();
    if active_fw.is_empty() {
        sections.push(Section {
            title: "防火墙".to_string(),
            icon: Some("🧱".to_string()),
            description: None,
            items: vec![Item::Warning {
                text: "未检测到活跃的防火墙".to_string(),
            }],
        });
    } else {
        sections.push(Section {
            title: "防火墙".to_string(),
            icon: Some("🧱".to_string()),
            description: None,
            items: active_fw,
        });
    }

    // 开放端口
    let ports = get_open_ports();
    if !ports.is_empty() {
        let rows: Vec<Vec<String>> = ports
            .iter()
            .take(20)
            .map(|p| vec![p.0.clone(), p.1.clone(), p.2.clone()])
            .collect();

        sections.push(Section {
            title: "开放端口".to_string(),
            icon: Some("🔌".to_string()),
            description: Some(format!("显示前 {} 个", rows.len().min(20))),
            items: vec![Item::Table {
                headers: vec!["端口".to_string(), "协议".to_string(), "进程".to_string()],
                rows,
                status: None,
            }],
        });
    }

    // 内核安全参数
    let kernel_params = check_kernel_security();
    if !kernel_params.is_empty() {
        sections.push(Section {
            title: "内核安全参数".to_string(),
            icon: Some("⚙️".to_string()),
            description: None,
            items: kernel_params,
        });
    }

    CheckResult {
        id: "security".to_string(),
        name: "安全策略检查".to_string(),
        description: "SELinux/AppArmor/防火墙/内核加固".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}

fn check_firewall_tool(cmd: &str, args: &[&str]) -> Option<Item> {
    let output = std::process::Command::new(cmd).args(args).output().ok()?;
    if output.status.success() {
        Some(Item::Label {
            key: cmd.to_string(),
            value: "已启用".to_string(),
            status: Some("ok".to_string()),
        })
    } else {
        None
    }
}

fn get_open_ports() -> Vec<(String, String, String)> {
    let output = std::process::Command::new("ss")
        .args(["-tulnp"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut ports = Vec::new();
    for line in output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            let local = parts[4];
            let port = local.rsplit(':').next().unwrap_or("-");
            let proto = parts[0];
            let process = parts.last().unwrap_or(&"-");
            let process = process.trim_start_matches("users:((").trim_end_matches(')');
            ports.push((port.to_string(), proto.to_string(), process.to_string()));
        }
    }
    ports
}

fn check_kernel_security() -> Vec<Item> {
    let params = vec![
        ("net.ipv4.tcp_syncookies", "SYN Cookie 保护"),
        ("net.ipv4.conf.all.rp_filter", "反向路径过滤"),
        ("net.ipv4.conf.all.accept_redirects", "ICMP 重定向"),
        ("net.ipv4.conf.all.send_redirects", "发送重定向"),
        ("net.ipv4.conf.all.accept_source_route", "源路由"),
        ("kernel.randomize_va_space", "ASLR"),
        ("kernel.exec-shield", "Exec-Shield"),
    ];

    let mut items = Vec::new();
    for (param, desc) in params {
        let value = read_sysctl(param);
        let status = match param {
            "net.ipv4.tcp_syncookies" => {
                if value == "1" {
                    "ok"
                } else {
                    "warn"
                }
            }
            "net.ipv4.conf.all.rp_filter" => {
                if value == "1" {
                    "ok"
                } else {
                    "warn"
                }
            }
            "kernel.randomize_va_space" => {
                if value == "2" {
                    "ok"
                } else if value == "1" {
                    "warn"
                } else {
                    "error"
                }
            }
            _ => "info",
        };
        items.push(Item::Label {
            key: desc.to_string(),
            value: format!("{} = {}", param, value),
            status: Some(status.to_string()),
        });
    }
    items
}

fn read_sysctl(param: &str) -> String {
    std::fs::read_to_string(format!("/proc/sys/{}", param.replace('.', "/")))
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "未知".to_string())
}
