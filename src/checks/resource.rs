use super::*;

pub fn check() -> CheckResult {
    let sys = crate::dashboard::get_system_info();
    let mut sections = vec![];

    let mut cpu_items = vec![
        Item::Label {
            key: "型号".to_string(),
            value: sys.cpu_brand.clone(),
            status: None,
        },
        Item::Label {
            key: "核心数".to_string(),
            value: sys.cpu_count.to_string(),
            status: None,
        },
        Item::Bar {
            key: "使用率".to_string(),
            value: sys.cpu_usage as f64,
            max: 100.0,
            unit: "%".to_string(),
            status: Some(bar_status(sys.cpu_usage as f64)),
        },
    ];

    for (idx, process) in sys.top_processes.iter().take(8).enumerate() {
        cpu_items.push(Item::Label {
            key: format!("Top {} CPU", idx + 1),
            value: format!(
                "{} pid={} cpu={:.1}% mem={}",
                process.name,
                process.pid,
                process.cpu_usage,
                fmt_bytes(process.memory_bytes)
            ),
            status: Some(
                if process.cpu_usage > 80.0 {
                    "warn"
                } else {
                    "ok"
                }
                .to_string(),
            ),
        });
    }

    sections.push(Section {
        title: "CPU 详情".to_string(),
        icon: Some("cpu".to_string()),
        description: None,
        items: cpu_items,
    });

    sections.push(Section {
        title: "内存详情".to_string(),
        icon: Some("memory-stick".to_string()),
        description: None,
        items: vec![
            Item::Bar {
                key: "物理内存".to_string(),
                value: sys.memory_usage as f64,
                max: 100.0,
                unit: "%".to_string(),
                status: Some(bar_status(sys.memory_usage as f64)),
            },
            Item::Label {
                key: "物理内存".to_string(),
                value: format!(
                    "{}/{}",
                    fmt_bytes(sys.memory_used),
                    fmt_bytes(sys.memory_total)
                ),
                status: None,
            },
            Item::Bar {
                key: "Swap".to_string(),
                value: sys.swap_usage as f64,
                max: 100.0,
                unit: "%".to_string(),
                status: Some(bar_status(sys.swap_usage as f64)),
            },
            Item::Label {
                key: "Swap".to_string(),
                value: format!("{}/{}", fmt_bytes(sys.swap_used), fmt_bytes(sys.swap_total)),
                status: None,
            },
        ],
    });

    let mut disk_items = Vec::new();
    for d in &sys.disks {
        disk_items.push(Item::Bar {
            key: format!("{} ({})", d.mount_point, d.fs_type),
            value: d.usage as f64,
            max: 100.0,
            unit: "%".to_string(),
            status: Some(bar_status(d.usage as f64)),
        });
        disk_items.push(Item::Label {
            key: d.mount_point.clone(),
            value: format!("{}/{}", fmt_bytes(d.used), fmt_bytes(d.total)),
            status: None,
        });
    }

    if !disk_items.is_empty() {
        sections.push(Section {
            title: "磁盘详情".to_string(),
            icon: Some("hard-drive".to_string()),
            description: None,
            items: disk_items,
        });
    }

    CheckResult {
        id: "resource".to_string(),
        name: "硬件资源".to_string(),
        description: "CPU/内存/磁盘/Swap/IO".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
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
