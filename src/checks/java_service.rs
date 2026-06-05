use super::common::*;
use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    pid: u32,
    name: String,
    cpu_usage: f32,
    memory_mb: f64,
    threads: String,
    cmd: String,
}

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("java-service");
    let prefix = cfg.service_prefix.trim().to_lowercase();
    let port_index = collect_listen_port_index();
    let mut rows = Vec::new();
    let mut log_sections = Vec::new();
    for proc_ in process_snapshot() {
        let lower = format!("{} {}", proc_.name.to_lowercase(), proc_.cmd.to_lowercase());
        if !is_java_process(&proc_.name, &proc_.cmd) {
            continue;
        }
        if !prefix.is_empty() && !lower.contains(&prefix) {
            continue;
        }
        let ports = port_index
            .get(&proc_.pid)
            .cloned()
            .unwrap_or_default()
            .join(",");
        rows.push(vec![
            proc_.pid.to_string(),
            proc_.name.clone(),
            format!("{:.1}", proc_.cpu_usage),
            format!("{:.1}", proc_.memory_mb),
            proc_.threads.clone(),
            ports,
            truncate(&proc_.cmd, 220),
        ]);
        if let Some(path) = infer_java_log_path(&cfg, &proc_.cmd) {
            log_sections.push(log_section(
                &format!("{} 异常日志 PID {}", proc_.name, proc_.pid),
                Some(path),
                80,
            ));
        }
    }
    let running = !rows.is_empty();
    let mut sections = vec![
        Section {
            title: "匹配配置".to_string(),
            icon: Some("JAVA".to_string()),
            description: Some(
                "规则引擎根据服务前缀、java/jar/tomcat 进程和日志关键词分析异常".to_string(),
            ),
            items: vec![
                label(
                    "服务前缀",
                    if cfg.service_prefix.is_empty() {
                        "未配置，显示全部 Java 服务"
                    } else {
                        &cfg.service_prefix
                    },
                    None,
                ),
                label(
                    "配置日志路径",
                    if cfg.log_path.is_empty() {
                        "未配置"
                    } else {
                        &cfg.log_path
                    },
                    None,
                ),
                label(
                    "匹配结果",
                    if running {
                        "发现 Java 服务"
                    } else {
                        "未发现 Java 服务"
                    },
                    Some(if running { "ok" } else { "warn" }),
                ),
            ],
        },
        table_section(
            "Java 服务列表",
            vec![
                "PID",
                "服务名",
                "CPU%",
                "内存MB",
                "线程数",
                "监听端口",
                "命令",
            ],
            rows,
            "未发现匹配的 Java 服务",
        ),
    ];
    sections.extend(log_sections);
    sections.push(table_section(
        "监听端口明细",
        vec!["协议", "本地地址", "对端", "进程"],
        listen_rows(&["java", "tomcat"], &[]),
        "未发现 Java 监听端口",
    ));
    sections.push(Section { title: "堆栈与线程建议".to_string(), icon: Some("STACK".to_string()), description: None, items: vec![
        Item::Info { text: "高 CPU 时建议执行 jstack <pid> 或 jcmd <pid> Thread.print，结合异常日志定位死锁/阻塞/外部依赖超时。".to_string() },
        Item::Info { text: "内存异常时建议执行 jcmd <pid> GC.heap_info、jmap -histo:live <pid>，必要时生成 heap dump。".to_string() },
    ]});
    CheckResult {
        id: "java-service".to_string(),
        name: "Java 服务常规检查".to_string(),
        description: "服务前缀匹配/异常日志/堆栈线程/CPU/内存/端口/运行状态".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(running),
        sections,
    }
}

fn process_snapshot() -> Vec<ProcessSnapshot> {
    let Some(out) = shell_output("ps -eo pid=,comm=,%cpu=,rss=,nlwp=,args=") else {
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
            let threads = parts.next().unwrap_or("-").to_string();
            let cmd = parts.collect::<Vec<_>>().join(" ");
            Some(ProcessSnapshot {
                pid,
                name,
                cpu_usage,
                memory_mb: rss_kb as f64 / 1024.0,
                threads,
                cmd,
            })
        })
        .collect()
}

fn is_java_process(name: &str, cmd: &str) -> bool {
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
    })
}

fn collect_listen_port_index() -> HashMap<u32, Vec<String>> {
    let Some(out) = shell_output("ss -ltnp 2>/dev/null") else {
        return HashMap::new();
    };
    let mut index: HashMap<u32, HashSet<String>> = HashMap::new();
    for line in out.lines() {
        let Some(port) = line
            .split_whitespace()
            .nth(3)
            .and_then(|local| local.rsplit(':').next())
            .filter(|port| *port != "0" && *port != "*")
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

fn infer_java_log_path(cfg: &EndpointConfig, cmd: &str) -> Option<std::path::PathBuf> {
    if !cfg.log_path.trim().is_empty() {
        return Some(std::path::PathBuf::from(&cfg.log_path));
    }
    for token in cmd.split_whitespace() {
        if token.contains("/logs/") || token.ends_with(".log") {
            let clean = token
                .trim_matches('"')
                .trim_matches('\'')
                .trim_start_matches("-Dlogging.file=")
                .trim_start_matches("-Dlogging.path=");
            let path = std::path::PathBuf::from(clean);
            if path.is_file() {
                return Some(path);
            }
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.extension().and_then(|v| v.to_str()) == Some("log") {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::is_java_process;

    #[test]
    fn java_process_matcher_accepts_real_java_forms() {
        assert!(is_java_process(
            "java",
            "/usr/bin/java -jar /data/apps/order-service.jar"
        ));
        assert!(is_java_process(
            "catalina.sh",
            "/opt/tomcat/bin/catalina.sh run"
        ));
        assert!(is_java_process(
            "worker",
            "/usr/lib/jvm/java-17/bin/java -Xmx2g com.example.Main"
        ));
    }

    #[test]
    fn java_process_matcher_rejects_dm_check_command() {
        assert!(!is_java_process("dm", "target/debug/dm check java-service"));
        assert!(!is_java_process(
            "bash",
            "/bin/bash -c cargo run --quiet -- check java-service"
        ));
    }
}
