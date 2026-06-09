use super::*;
use sysinfo::System;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // 系统信息
    let hostname = System::host_name().unwrap_or_else(|| "未知".to_string());
    let os = System::long_os_version().unwrap_or_else(|| "未知".to_string());
    let kernel = System::kernel_version().unwrap_or_else(|| "未知".to_string());
    let arch = std::env::consts::ARCH.to_string();

    sections.push(Section {
        title: "系统信息".to_string(),
        icon: Some("🖥️".to_string()),
        description: None,
        items: vec![
            Item::Label {
                key: "主机名".to_string(),
                value: hostname,
                status: None,
            },
            Item::Label {
                key: "操作系统".to_string(),
                value: os,
                status: None,
            },
            Item::Label {
                key: "内核版本".to_string(),
                value: kernel,
                status: None,
            },
            Item::Label {
                key: "架构".to_string(),
                value: arch,
                status: None,
            },
        ],
    });

    // Shell 环境
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "未知".to_string());
    let user = std::env::var("USER").unwrap_or_else(|_| "未知".to_string());
    let home = std::env::var("HOME").unwrap_or_else(|_| "未知".to_string());
    let path = std::env::var("PATH").unwrap_or_default();
    let path_rows: Vec<Vec<String>> = path
        .split(':')
        .filter(|entry| !entry.trim().is_empty())
        .enumerate()
        .map(|(index, entry)| vec![(index + 1).to_string(), entry.to_string()])
        .collect();
    let mut env_rows: Vec<Vec<String>> = std::env::vars()
        .filter(|(key, _)| key != "PATH")
        .map(|(key, value)| vec![key, value])
        .collect();
    env_rows.sort_by(|a, b| a[0].cmp(&b[0]));

    sections.push(Section {
        title: "Shell 环境".to_string(),
        icon: Some("🐚".to_string()),
        description: Some(
            "PATH 单独拆分为路径列表；其他环境变量默认折叠，按需展开查看。".to_string(),
        ),
        items: vec![
            Item::Label {
                key: "当前用户".to_string(),
                value: user,
                status: None,
            },
            Item::Label {
                key: "默认 Shell".to_string(),
                value: shell,
                status: None,
            },
            Item::Label {
                key: "主目录".to_string(),
                value: home,
                status: None,
            },
            Item::Table {
                headers: vec!["序号".to_string(), "PATH 路径".to_string()],
                rows: path_rows,
                status: None,
            },
            Item::Table {
                headers: vec!["变量".to_string(), "值".to_string()],
                rows: env_rows,
                status: Some("collapsed".to_string()),
            },
        ],
    });

    // 语言环境
    let lang = std::env::var("LANG").unwrap_or_else(|_| "未知".to_string());
    let lc_all = std::env::var("LC_ALL").unwrap_or_else(|_| "未设置".to_string());

    sections.push(Section {
        title: "语言环境".to_string(),
        icon: Some("🌐".to_string()),
        description: None,
        items: vec![
            Item::Label {
                key: "LANG".to_string(),
                value: lang,
                status: None,
            },
            Item::Label {
                key: "LC_ALL".to_string(),
                value: lc_all,
                status: None,
            },
        ],
    });

    // 时区
    let tz = std::env::var("TZ").unwrap_or_else(|_| "未设置".to_string());
    let now = chrono::Local::now();

    sections.push(Section {
        title: "时间".to_string(),
        icon: Some("⏰".to_string()),
        description: None,
        items: vec![
            Item::Label {
                key: "时区".to_string(),
                value: tz,
                status: None,
            },
            Item::Label {
                key: "当前时间".to_string(),
                value: now.format("%Y-%m-%d %H:%M:%S").to_string(),
                status: None,
            },
            Item::Label {
                key: "UTC 时间".to_string(),
                value: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                status: None,
            },
        ],
    });

    CheckResult {
        id: "environment".to_string(),
        name: "环境信息".to_string(),
        description: "系统/变量/软件/用户".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}
