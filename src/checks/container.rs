use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // Docker 检查
    let docker_ok = std::process::Command::new("docker")
        .args(["info"])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if docker_ok {
        // 容器列表
        let output = std::process::Command::new("docker")
            .args([
                "ps",
                "-a",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}",
            ])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let mut rows = Vec::new();
        for line in output.lines().take(20) {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                rows.push(vec![
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                    parts[3].to_string(),
                ]);
            }
        }

        sections.push(Section {
            title: "Docker 容器".to_string(),
            icon: Some("🐳".to_string()),
            description: None,
            items: vec![Item::Table {
                headers: vec![
                    "ID".to_string(),
                    "名称".to_string(),
                    "状态".to_string(),
                    "镜像".to_string(),
                ],
                rows,
                status: None,
            }],
        });

        // 镜像列表
        let output = std::process::Command::new("docker")
            .args([
                "images",
                "--format",
                "{{.Repository}}:{{.Tag}}\t{{.Size}}\t{{.CreatedSince}}",
            ])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let mut rows = Vec::new();
        for line in output.lines().take(10) {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                rows.push(vec![
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                ]);
            }
        }

        sections.push(Section {
            title: "Docker 镜像".to_string(),
            icon: Some("💿".to_string()),
            description: None,
            items: vec![Item::Table {
                headers: vec![
                    "镜像".to_string(),
                    "大小".to_string(),
                    "创建时间".to_string(),
                ],
                rows,
                status: None,
            }],
        });
    } else {
        sections.push(Section {
            title: "Docker".to_string(),
            icon: Some("🐳".to_string()),
            description: None,
            items: vec![Item::Info {
                text: "Docker 未安装或未运行".to_string(),
            }],
        });
    }

    CheckResult {
        id: "container".to_string(),
        name: "容器状态".to_string(),
        description: "Docker容器/镜像/状态".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}
