use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub index: usize,
    pub pid: u32,
    pub name: String,
    pub process_name: String,
    pub process_path: String,
    pub listen_ports: Vec<String>,
    pub status: String,
    pub cpu_usage: f32,
    pub memory_mb: f64,
    pub priority: u8,
    pub category: String,
}

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    pid: u32,
    name: String,
    cmd: String,
    cpu_usage: f32,
    memory_mb: f64,
}

fn clean_unit_name(value: &str) -> Option<String> {
    let unit = value.trim().trim_matches('"').trim_matches('\'');
    if unit.is_empty() || unit == "-" {
        return None;
    }
    if unit.ends_with(".service")
        || unit.ends_with(".scope")
        || unit.ends_with(".timer")
        || unit.ends_with(".socket")
    {
        Some(unit.to_string())
    } else {
        None
    }
}

fn unit_display_name(unit: &str, fallback: &str) -> String {
    if unit.trim().is_empty() {
        return fallback.to_string();
    }
    unit.trim()
        .trim_end_matches(".service")
        .trim_end_matches(".scope")
        .trim_end_matches(".timer")
        .trim_end_matches(".socket")
        .to_string()
}

fn classify_service(name: &str, cmd: &str) -> (u8, String) {
    let lower = format!("{} {}", name.to_lowercase(), cmd.to_lowercase());
    if is_java_like_process(name, cmd) {
        return (1, "Java 服务".to_string());
    }
    if lower.contains("nginx") {
        return (2, "Nginx".to_string());
    }
    if lower.contains("redis") {
        return (2, "Redis".to_string());
    }
    if lower.contains("mysql") || lower.contains("mariadb") || lower.contains("mysqld") {
        return (2, "MySQL".to_string());
    }
    if lower.contains("postgres") {
        return (2, "PostgreSQL".to_string());
    }
    if lower.contains("mongo") {
        return (2, "MongoDB".to_string());
    }
    if lower.contains("kafka") {
        return (2, "Kafka".to_string());
    }
    if lower.contains("elastic") || lower.contains("kibana") {
        return (2, "Elasticsearch".to_string());
    }
    if lower.contains("rabbit") {
        return (2, "RabbitMQ".to_string());
    }
    if lower.contains("docker") || lower.contains("containerd") {
        return (2, "Docker".to_string());
    }
    if lower.contains("caddy") {
        return (2, "Caddy".to_string());
    }
    if lower.contains("haproxy") {
        return (2, "HAProxy".to_string());
    }
    if lower.contains("memcached") {
        return (2, "Memcached".to_string());
    }
    if lower.contains("zookeeper") {
        return (2, "ZooKeeper".to_string());
    }
    if lower.contains("consul") {
        return (2, "Consul".to_string());
    }
    if lower.contains("etcd") {
        return (2, "etcd".to_string());
    }
    if lower.contains("sshd") {
        return (3, "SSH".to_string());
    }
    if lower.contains("systemd") {
        return (3, "Systemd".to_string());
    }
    if lower.contains("cron") {
        return (3, "Cron".to_string());
    }
    if lower.contains("rsyslog") || lower.contains("syslog") {
        return (3, "Syslog".to_string());
    }
    if lower.contains("dbus") {
        return (3, "D-Bus".to_string());
    }
    if lower.contains("networkd") || lower.contains("NetworkManager") {
        return (3, "网络".to_string());
    }
    if lower.contains("sshd") || lower.contains("ssh") {
        return (3, "SSH".to_string());
    }
    if lower.contains("node") || lower.contains("npm") {
        return (2, "Node.js".to_string());
    }
    if lower.contains("python") || lower.contains("gunicorn") {
        return (2, "Python".to_string());
    }
    (4, "其他".to_string())
}

fn is_java_like_process(name: &str, cmd: &str) -> bool {
    let lower_name = name.to_lowercase();
    if lower_name == "java"
        || lower_name == "java.exe"
        || lower_name.contains("tomcat")
        || lower_name.contains("catalina")
    {
        return true;
    }
    cmd.split_whitespace().any(|token| {
        let clean = token
            .trim_matches('"')
            .trim_matches('\'')
            .trim_matches('(')
            .trim_matches(')');
        let lower = clean.to_lowercase();
        lower == "java"
            || lower.ends_with("/java")
            || lower.ends_with("\\java.exe")
            || lower.ends_with(".jar")
            || lower.contains("/tomcat/")
            || lower.contains("/catalina.")
            || lower.contains("org.springframework.boot.loader")
    })
}

fn collect_listen_port_index() -> HashMap<u32, Vec<String>> {
    let output = std::process::Command::new("ss")
        .args(["-tulnp"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut index: HashMap<u32, HashSet<String>> = HashMap::new();
    for line in output.lines() {
        let local = line
            .split_whitespace()
            .nth(4)
            .or_else(|| line.split_whitespace().nth(3));
        let Some(port) = local
            .and_then(|v| v.rsplit(':').next())
            .filter(|v| *v != "0" && *v != "*")
        else {
            continue;
        };

        let mut from = 0usize;
        while let Some(offset) = line[from..].find("pid=") {
            let start = from + offset + 4;
            let pid_text: String = line[start..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(pid) = pid_text.parse::<u32>() {
                index.entry(pid).or_default().insert(port.to_string());
            }
            from = start.saturating_add(pid_text.len());
            if from >= line.len() {
                break;
            }
        }
    }

    index
        .into_iter()
        .map(|(pid, ports)| {
            let mut ports: Vec<String> = ports.into_iter().collect();
            ports.sort();
            ports.dedup();
            (pid, ports)
        })
        .collect()
}

fn collect_systemd_unit_index() -> HashMap<u32, String> {
    let mut index = HashMap::new();

    if let Some(out) = crate::checks::common::shell_output(
        "systemctl show --all --property=Id,MainPID,ControlPID --value '*' 2>/dev/null",
    ) {
        let mut current_unit: Option<String> = None;
        for line in out.lines() {
            if let Some(unit) = clean_unit_name(line) {
                current_unit = Some(unit);
                continue;
            }
            let Ok(pid) = line.trim().parse::<u32>() else {
                continue;
            };
            if pid > 0 {
                if let Some(unit) = &current_unit {
                    index.entry(pid).or_insert_with(|| unit.clone());
                }
            }
        }
    }

    for proc in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let pid = proc
            .file_name()
            .to_string_lossy()
            .parse::<u32>()
            .ok()
            .unwrap_or(0);
        if pid == 0 || index.contains_key(&pid) {
            continue;
        }
        let cgroup_path = proc.path().join("cgroup");
        let Ok(content) = std::fs::read_to_string(cgroup_path) else {
            continue;
        };
        if let Some(unit) = infer_unit_from_cgroup(&content) {
            index.insert(pid, unit);
        }
    }

    index
}

fn infer_unit_from_cgroup(content: &str) -> Option<String> {
    for line in content.lines() {
        for raw in line.split('/') {
            let decoded = raw.replace("\\x2d", "-");
            if let Some(unit) = clean_unit_name(&decoded) {
                return Some(unit);
            }
        }
    }
    None
}

fn get_listen_ports(pid: u32, port_index: &HashMap<u32, Vec<String>>) -> Vec<String> {
    port_index.get(&pid).cloned().unwrap_or_default()
}

fn process_snapshot() -> Vec<ProcessSnapshot> {
    let Some(out) = crate::checks::common::shell_output("ps -eo pid=,comm=,%cpu=,rss=,args=")
    else {
        return Vec::new();
    };

    out.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse::<u32>().ok()?;
            let name = parts.next()?.to_string();
            let cpu_usage = parts
                .next()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0);
            let rss_kb = parts
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let cmd = parts.collect::<Vec<_>>().join(" ");
            Some(ProcessSnapshot {
                pid,
                name,
                cmd,
                cpu_usage,
                memory_mb: rss_kb as f64 / 1024.0,
            })
        })
        .collect()
}

pub fn check() -> CheckResult {
    let port_index = collect_listen_port_index();
    let unit_index = collect_systemd_unit_index();

    let mut service_map: std::collections::HashMap<String, ServiceInfo> =
        std::collections::HashMap::new();
    let mut index = 0;

    for proc in process_snapshot() {
        let (priority, category) = classify_service(&proc.name, &proc.cmd);
        let unit = unit_index.get(&proc.pid).cloned();
        let service_key = unit.clone().unwrap_or_else(|| proc.name.clone());
        let display_name = unit
            .as_deref()
            .map(|u| unit_display_name(u, &proc.name))
            .unwrap_or_else(|| proc.name.clone());
        let process_path = if let Some(unit) = unit.as_deref() {
            format!("{} | {}", unit, proc.cmd)
        } else {
            proc.cmd.clone()
        };

        let ports = get_listen_ports(proc.pid, &port_index);

        if priority > 3 && ports.is_empty() {
            continue;
        }

        if let Some(existing) = service_map.get_mut(&service_key) {
            existing.cpu_usage += proc.cpu_usage;
            existing.memory_mb += proc.memory_mb;
            if existing.pid == 0 || proc.pid < existing.pid {
                existing.pid = proc.pid;
            }
            for port in ports {
                if !existing.listen_ports.contains(&port) {
                    existing.listen_ports.push(port);
                }
            }
        } else {
            index += 1;
            service_map.insert(
                service_key,
                ServiceInfo {
                    index,
                    pid: proc.pid,
                    name: display_name,
                    process_name: proc.name,
                    process_path,
                    listen_ports: ports,
                    status: "运行中".to_string(),
                    cpu_usage: proc.cpu_usage,
                    memory_mb: proc.memory_mb,
                    priority,
                    category,
                },
            );
        }
    }

    let mut services: Vec<ServiceInfo> = service_map.into_values().collect();
    services.sort_by(|a, b| {
        a.priority.cmp(&b.priority).then(
            b.cpu_usage
                .partial_cmp(&a.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    let total = services.len();
    let running = services.iter().filter(|s| s.status == "运行中").count();
    let java_count = services.iter().filter(|s| s.priority == 1).count();
    let middleware_count = services.iter().filter(|s| s.priority == 2).count();
    let system_count = services.iter().filter(|s| s.priority == 3).count();

    let mut sections = vec![];

    sections.push(Section {
        title: "服务汇总".to_string(),
        icon: Some("📊".to_string()),
        description: None,
        items: vec![
            Item::Label {
                key: "总服务数".to_string(),
                value: total.to_string(),
                status: None,
            },
            Item::Label {
                key: "运行中".to_string(),
                value: running.to_string(),
                status: Some("ok".to_string()),
            },
            Item::Label {
                key: "Java 服务".to_string(),
                value: java_count.to_string(),
                status: None,
            },
            Item::Label {
                key: "中间件".to_string(),
                value: middleware_count.to_string(),
                status: None,
            },
            Item::Label {
                key: "系统服务".to_string(),
                value: system_count.to_string(),
                status: None,
            },
        ],
    });

    let mut rows: Vec<Vec<String>> = Vec::new();
    for s in &services {
        let ports_str = if s.listen_ports.is_empty() {
            "-".to_string()
        } else {
            s.listen_ports.join(", ")
        };
        let status_icon = if s.status == "运行中" {
            "✓"
        } else {
            "✗"
        };
        rows.push(vec![
            s.index.to_string(),
            s.pid.to_string(),
            s.name.clone(),
            s.process_name.clone(),
            s.process_path.clone(),
            ports_str,
            format!("{} {}", status_icon, s.status),
            format!("{:.1}%", s.cpu_usage),
            format!("{:.1}MB", s.memory_mb),
            s.category.clone(),
        ]);
    }

    if !rows.is_empty() {
        sections.push(Section {
            title: "服务列表".to_string(),
            icon: Some("🔧".to_string()),
            description: Some(format!("{} 个服务，按类型优先级排序", total)),
            items: vec![Item::Table {
                headers: vec![
                    "#".to_string(),
                    "PID".to_string(),
                    "服务名".to_string(),
                    "进程".to_string(),
                    "进程路径".to_string(),
                    "端口".to_string(),
                    "状态".to_string(),
                    "CPU".to_string(),
                    "内存".to_string(),
                    "类型".to_string(),
                ],
                rows,
                status: None,
            }],
        });
    }

    CheckResult {
        id: "service-manage".to_string(),
        name: "服务管理".to_string(),
        description: "管理系统内所有服务：Java优先 > 中间件 > 系统进程".to_string(),
        category: "服务管理".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: CheckStatus::Ok,
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_systemd_unit_from_cgroup() {
        let content = "0::/system.slice/nginx.service\n";
        assert_eq!(
            infer_unit_from_cgroup(content).as_deref(),
            Some("nginx.service")
        );
    }

    #[test]
    fn unit_display_name_removes_suffix() {
        assert_eq!(
            unit_display_name("redis-server.service", "redis"),
            "redis-server"
        );
        assert_eq!(unit_display_name("", "java"), "java");
    }
}
