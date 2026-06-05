use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // Crontab
    let cron_output = std::process::Command::new("crontab")
        .args(["-l"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let cron_lines: Vec<&str> = cron_output
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .collect();

    sections.push(Section {
        title: "用户 Crontab".to_string(),
        icon: Some("⏰".to_string()),
        description: None,
        items: if cron_lines.is_empty() {
            vec![Item::Info {
                text: "无定时任务".to_string(),
            }]
        } else {
            vec![Item::Label {
                key: "定时任务数".to_string(),
                value: cron_lines.len().to_string(),
                status: Some("ok".to_string()),
            }]
        },
    });

    // 系统定时任务
    let sys_cron = std::fs::read_dir("/etc/cron.d")
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .count()
        })
        .unwrap_or(0);

    sections.push(Section {
        title: "系统定时任务".to_string(),
        icon: Some("📋".to_string()),
        description: None,
        items: vec![Item::Label {
            key: "cron.d 任务数".to_string(),
            value: sys_cron.to_string(),
            status: Some("ok".to_string()),
        }],
    });

    CheckResult {
        id: "schedule".to_string(),
        name: "定时任务".to_string(),
        description: "crontab/系统cron/服务状态".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}
