use super::*;

pub fn check() -> CheckResult {
    let sys = crate::dashboard::get_system_info();
    let load = &sys.load_avg;

    let mut sections = vec![
        Section {
            title: "系统基本信息".to_string(),
            icon: Some("system".to_string()),
            description: None,
            items: vec![
                Item::Label {
                    key: "主机名".to_string(),
                    value: sys.hostname.clone(),
                    status: None,
                },
                Item::Label {
                    key: "操作系统".to_string(),
                    value: sys.os.clone(),
                    status: None,
                },
                Item::Label {
                    key: "内核版本".to_string(),
                    value: sys.kernel.clone(),
                    status: None,
                },
                Item::Label {
                    key: "系统架构".to_string(),
                    value: sys.arch.clone(),
                    status: None,
                },
                Item::Label {
                    key: "CPU".to_string(),
                    value: format!("{} ({}核)", sys.cpu_brand, sys.cpu_count),
                    status: None,
                },
                Item::Label {
                    key: "运行时间".to_string(),
                    value: format_uptime(sys.uptime),
                    status: None,
                },
                Item::Label {
                    key: "进程数".to_string(),
                    value: sys.process_count.to_string(),
                    status: None,
                },
            ],
        },
        Section {
            title: "资源使用".to_string(),
            icon: Some("activity".to_string()),
            description: None,
            items: vec![
                Item::Bar {
                    key: "CPU".to_string(),
                    value: sys.cpu_usage as f64,
                    max: 100.0,
                    unit: "%".to_string(),
                    status: Some(bar_status(sys.cpu_usage as f64)),
                },
                Item::Bar {
                    key: "内存".to_string(),
                    value: sys.memory_usage as f64,
                    max: 100.0,
                    unit: "%".to_string(),
                    status: Some(bar_status(sys.memory_usage as f64)),
                },
                Item::Bar {
                    key: "Swap".to_string(),
                    value: sys.swap_usage as f64,
                    max: 100.0,
                    unit: "%".to_string(),
                    status: Some(bar_status(sys.swap_usage as f64)),
                },
                Item::Bar {
                    key: "磁盘".to_string(),
                    value: sys.disk_usage as f64,
                    max: 100.0,
                    unit: "%".to_string(),
                    status: Some(bar_status(sys.disk_usage as f64)),
                },
                Item::Label {
                    key: "内存详情".to_string(),
                    value: format!(
                        "{}/{}",
                        fmt_bytes(sys.memory_used),
                        fmt_bytes(sys.memory_total)
                    ),
                    status: None,
                },
                Item::Label {
                    key: "磁盘详情".to_string(),
                    value: format!("{}/{}", fmt_bytes(sys.disk_used), fmt_bytes(sys.disk_total)),
                    status: None,
                },
            ],
        },
        Section {
            title: "系统负载".to_string(),
            icon: Some("gauge".to_string()),
            description: None,
            items: vec![
                Item::Label {
                    key: "1分钟".to_string(),
                    value: format!("{:.2}", load.one),
                    status: Some(load_status(load.one, sys.cpu_count as f64)),
                },
                Item::Label {
                    key: "5分钟".to_string(),
                    value: format!("{:.2}", load.five),
                    status: Some(load_status(load.five, sys.cpu_count as f64)),
                },
                Item::Label {
                    key: "15分钟".to_string(),
                    value: format!("{:.2}", load.fifteen),
                    status: Some(load_status(load.fifteen, sys.cpu_count as f64)),
                },
            ],
        },
    ];

    let net_items: Vec<Item> = sys
        .networks
        .iter()
        .filter(|n| {
            n.name != "lo"
                && !n.name.starts_with("docker")
                && !n.name.starts_with("br-")
                && !n.name.starts_with("veth")
        })
        .map(|n| Item::Label {
            key: n.name.clone(),
            value: format!(
                "↓{} ↑{}",
                fmt_bytes(n.received_bytes),
                fmt_bytes(n.transmitted_bytes)
            ),
            status: None,
        })
        .collect();

    if !net_items.is_empty() {
        sections.push(Section {
            title: "网络接口".to_string(),
            icon: Some("network".to_string()),
            description: None,
            items: net_items,
        });
    }

    let disk_items: Vec<Item> = sys
        .disks
        .iter()
        .map(|d| Item::Bar {
            key: format!("{} ({})", d.mount_point, d.fs_type),
            value: d.usage as f64,
            max: 100.0,
            unit: "%".to_string(),
            status: Some(bar_status(d.usage as f64)),
        })
        .collect();

    if !disk_items.is_empty() {
        sections.push(Section {
            title: "磁盘挂载".to_string(),
            icon: Some("hard-drive".to_string()),
            description: None,
            items: disk_items,
        });
    }

    CheckResult {
        id: "system".to_string(),
        name: "系统综合检查".to_string(),
        description: "主机信息/CPU/内存/磁盘/网络/进程".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}

fn format_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    format!("{}天 {}时 {}分", d, h, m)
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

fn bar_status(v: f64) -> String {
    if v < 60.0 {
        "ok".to_string()
    } else if v < 80.0 {
        "warn".to_string()
    } else {
        "error".to_string()
    }
}

fn load_status(load: f64, cores: f64) -> String {
    let ratio = if cores > 0.0 { load / cores } else { load };
    if ratio < 0.7 {
        "ok".to_string()
    } else if ratio < 1.0 {
        "warn".to_string()
    } else {
        "error".to_string()
    }
}
