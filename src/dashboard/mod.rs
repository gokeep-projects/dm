use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

const TOP_PROCESS_LIMIT: usize = 20;

/// 系统信息
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_usage: f32,
    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_usage: f32,
    pub disk_total: u64,
    pub disk_used: u64,
    pub disk_usage: f32,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub cpu_count: usize,
    pub cpu_brand: String,
    pub uptime: u64,
    pub boot_time: u64,
    pub load_avg: LoadAvg,
    pub process_count: usize,
    pub networks: Vec<NetInterface>,
    pub disks: Vec<DiskInfo>,
    pub top_processes: Vec<ProcessInfo>,
}

#[derive(Debug, Serialize, Default)]
pub struct LoadAvg {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Debug, Serialize, Default)]
pub struct NetInterface {
    pub name: String,
    pub ip: String,
    pub mac: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
    pub received_packets: u64,
    pub transmitted_packets: u64,
    pub received_errors: u64,
    pub transmitted_errors: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub usage: f32,
    pub fs_type: String,
}

#[derive(Debug, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub ports: Vec<String>,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
}

#[cfg(target_os = "linux")]
fn read_loadavg() -> LoadAvg {
    use std::fs;
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 3 {
                Some(LoadAvg {
                    one: parts[0].parse().unwrap_or(0.0),
                    five: parts[1].parse().unwrap_or(0.0),
                    fifteen: parts[2].parse().unwrap_or(0.0),
                })
            } else {
                None
            }
        })
        .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
fn read_loadavg() -> LoadAvg {
    LoadAvg::default()
}

fn collect_networks() -> Vec<NetInterface> {
    let Ok(content) = fs::read_to_string("/proc/net/dev") else {
        return Vec::new();
    };
    content
        .lines()
        .skip(2)
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let values: Vec<&str> = rest.split_whitespace().collect();
            if values.len() < 16 {
                return None;
            }
            Some(NetInterface {
                name: name.trim().to_string(),
                ip: interface_ip(name.trim()),
                mac: interface_mac(name.trim()),
                received_bytes: values[0].parse().unwrap_or(0),
                received_packets: values[1].parse().unwrap_or(0),
                received_errors: values[2].parse().unwrap_or(0),
                transmitted_bytes: values[8].parse().unwrap_or(0),
                transmitted_packets: values[9].parse().unwrap_or(0),
                transmitted_errors: values[10].parse().unwrap_or(0),
            })
        })
        .collect()
}

fn interface_mac(name: &str) -> String {
    fs::read_to_string(format!("/sys/class/net/{}/address", name))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

fn interface_ip(name: &str) -> String {
    Command::new("ip")
        .args(["-o", "-4", "addr", "show", "dev", name])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .and_then(|stdout| {
            stdout
                .split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .find_map(|w| {
                    if w[0] == "inet" {
                        Some(w[1].split('/').next().unwrap_or("").to_string())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_default()
}

fn get_local_ip() -> String {
    if let Ok(output) = std::process::Command::new("hostname").args(["-I"]).output() {
        let ips = String::from_utf8_lossy(&output.stdout);
        for ip in ips.split_whitespace() {
            if !ip.starts_with("127.") && !ip.starts_with("172.") && !ip.contains(":") {
                return ip.to_string();
            }
        }
        if let Some(first) = ips.split_whitespace().next() {
            return first.to_string();
        }
    }
    String::new()
}

fn collect_disks() -> Vec<DiskInfo> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .iter()
        .map(|d| {
            let total = d.total_space();
            let used = total.saturating_sub(d.available_space());
            let usage = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            DiskInfo {
                name: d.name().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().to_string(),
                total,
                used,
                usage,
                fs_type: d.file_system().to_string_lossy().to_string(),
            }
        })
        .collect()
}

fn collect_top_processes(n: usize) -> Vec<ProcessInfo> {
    let port_index = collect_process_port_index();
    let output = Command::new("ps")
        .args(["-eo", "pid=,comm=,%cpu=,rss=,args=", "--sort=-%cpu"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut procs: Vec<ProcessInfo> = output
        .lines()
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
            Some(ProcessInfo {
                pid,
                name,
                path: infer_process_path(pid, &cmd),
                ports: port_index.get(&pid).cloned().unwrap_or_default(),
                cpu_usage,
                memory_bytes: rss_kb.saturating_mul(1024),
            })
        })
        .collect();
    // CPU + 内存综合排序 (CPU权重0.6，内存权重0.4)
    let max_cpu = procs
        .iter()
        .map(|p| p.cpu_usage)
        .fold(0.0f32, f32::max)
        .max(1.0);
    let max_mem = procs
        .iter()
        .map(|p| p.memory_bytes)
        .fold(0u64, u64::max)
        .max(1);
    procs.sort_by(|a, b| {
        let score_a =
            (a.cpu_usage / max_cpu) * 0.6 + (a.memory_bytes as f32 / max_mem as f32) * 0.4;
        let score_b =
            (b.cpu_usage / max_cpu) * 0.6 + (b.memory_bytes as f32 / max_mem as f32) * 0.4;
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    procs.truncate(n);
    procs
}

fn collect_process_port_index() -> HashMap<u32, Vec<String>> {
    let output = Command::new("ss")
        .args(["-tulnp"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    parse_process_port_index(&output)
}

fn parse_process_port_index(output: &str) -> HashMap<u32, Vec<String>> {
    let mut index: HashMap<u32, HashSet<String>> = HashMap::new();
    for line in output.lines() {
        let mut columns = line.split_whitespace();
        let proto = columns.next().unwrap_or_default().to_ascii_lowercase();
        if proto != "tcp" && proto != "udp" {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(local) = fields.get(4).or_else(|| fields.get(3)).copied() else {
            continue;
        };
        let Some(port) = local
            .rsplit_once(':')
            .map(|(_, port)| port.trim_matches(']'))
            .filter(|port| !port.is_empty() && *port != "0" && *port != "*")
        else {
            continue;
        };
        let listen = format!("{proto}:{port}");

        let mut from = 0usize;
        while let Some(offset) = line[from..].find("pid=") {
            let start = from + offset + 4;
            let pid_text: String = line[start..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(pid) = pid_text.parse::<u32>() {
                index.entry(pid).or_default().insert(listen.clone());
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

fn infer_process_path(pid: u32, cmd: &str) -> String {
    if let Ok(path) = fs::read_link(format!("/proc/{pid}/exe")) {
        let text = path.to_string_lossy().to_string();
        if !text.is_empty() {
            return text;
        }
    }

    cmd.split_whitespace()
        .map(|token| {
            token
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches('(')
                .trim_matches(')')
                .to_string()
        })
        .find(|token| token.starts_with('/') && !token.starts_with("/proc/"))
        .filter(|token| !token.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn read_meminfo_value(key: &str) -> u64 {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                let (name, rest) = line.split_once(':')?;
                if name != key {
                    return None;
                }
                rest.split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|kb| kb.saturating_mul(1024))
            })
        })
        .unwrap_or(0)
}

fn read_cpu_stat() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().find(|line| line.starts_with("cpu "))?;
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse::<u64>().ok())
        .collect();
    if values.len() < 4 {
        return None;
    }
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let total = values.iter().sum();
    Some((idle, total))
}

fn read_cpu_usage() -> f32 {
    let Some((idle_a, total_a)) = read_cpu_stat() else {
        return 0.0;
    };
    thread::sleep(Duration::from_millis(80));
    let Some((idle_b, total_b)) = read_cpu_stat() else {
        return 0.0;
    };
    let idle_delta = idle_b.saturating_sub(idle_a);
    let total_delta = total_b.saturating_sub(total_a);
    if total_delta == 0 {
        return 0.0;
    }
    ((total_delta.saturating_sub(idle_delta)) as f32 / total_delta as f32) * 100.0
}

fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn cpu_brand() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                if key.trim() == "model name" || key.trim() == "Hardware" {
                    Some(value.trim().to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "未知".to_string())
}

fn process_count() -> usize {
    fs::read_dir("/proc")
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .chars()
                        .all(|c| c.is_ascii_digit())
                })
                .count()
        })
        .unwrap_or(0)
}

fn read_uptime() -> u64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|content| content.split_whitespace().next()?.parse::<f64>().ok())
        .map(|v| v as u64)
        .unwrap_or(0)
}

fn read_boot_time() -> u64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                line.strip_prefix("btime ")
                    .and_then(|v| v.trim().parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}

fn hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| fs::read_to_string("/etc/hostname"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知".to_string())
}

fn os_version() -> String {
    fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "未知".to_string())
}

fn kernel_version() -> String {
    Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知".to_string())
}

/// 获取系统信息
pub fn get_system_info() -> SystemInfo {
    let cpu_usage = read_cpu_usage();
    let cpu_count = cpu_count();
    let cpu_brand = cpu_brand();

    let memory_total = read_meminfo_value("MemTotal");
    let memory_available = read_meminfo_value("MemAvailable");
    let memory_used = memory_total.saturating_sub(memory_available);
    let memory_usage = if memory_total > 0 {
        (memory_used as f32 / memory_total as f32) * 100.0
    } else {
        0.0
    };

    let swap_total = read_meminfo_value("SwapTotal");
    let swap_free = read_meminfo_value("SwapFree");
    let swap_used = swap_total.saturating_sub(swap_free);
    let swap_usage = if swap_total > 0 {
        (swap_used as f32 / swap_total as f32) * 100.0
    } else {
        0.0
    };

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let disk_total: u64 = disks.iter().map(|d| d.total_space()).sum();
    let disk_used: u64 = disks
        .iter()
        .map(|d| d.total_space() - d.available_space())
        .sum();
    let disk_usage = if disk_total > 0 {
        (disk_used as f32 / disk_total as f32) * 100.0
    } else {
        0.0
    };

    let mut networks = collect_networks();
    let local_ip = get_local_ip();
    if !local_ip.is_empty() {
        for n in networks.iter_mut() {
            if n.ip.is_empty() && !n.name.contains("lo") && !n.name.contains("docker") {
                n.ip = local_ip.clone();
                break;
            }
        }
    }

    SystemInfo {
        cpu_usage,
        memory_total,
        memory_used,
        memory_usage,
        swap_total,
        swap_used,
        swap_usage,
        disk_total,
        disk_used,
        disk_usage,
        hostname: hostname(),
        os: os_version(),
        kernel: kernel_version(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_count,
        cpu_brand,
        uptime: read_uptime(),
        boot_time: read_boot_time(),
        load_avg: read_loadavg(),
        process_count: process_count(),
        networks,
        disks: collect_disks(),
        top_processes: collect_top_processes(TOP_PROCESS_LIMIT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_process_port_index_extracts_and_deduplicates_pid_ports() {
        let sample = r#"
Netid State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process
tcp   LISTEN 0      511          0.0.0.0:8080      0.0.0.0:* users:(("java",pid=123,fd=7))
tcp   LISTEN 0      511          0.0.0.0:8080      0.0.0.0:* users:(("java",pid=123,fd=8))
udp   UNCONN 0      0               [::]:53           [::]:* users:(("named",pid=77,fd=3))
tcp   LISTEN 0      128        127.0.0.1:9090      0.0.0.0:* users:(("dm",pid=123,fd=12))
"#;

        let index = parse_process_port_index(sample);

        assert_eq!(
            index.get(&123).cloned().unwrap_or_default(),
            vec!["tcp:8080".to_string(), "tcp:9090".to_string()]
        );
        assert_eq!(
            index.get(&77).cloned().unwrap_or_default(),
            vec!["udp:53".to_string()]
        );
    }

    #[test]
    fn infer_process_path_falls_back_to_absolute_command_token() {
        let path = infer_process_path(u32::MAX, "java -Xmx1g -jar /opt/dm/app.jar");

        assert_eq!(path, "/opt/dm/app.jar");
    }
}
