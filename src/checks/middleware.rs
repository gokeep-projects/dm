use super::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Clone)]
struct MiddlewareSpec {
    id: &'static str,
    name: &'static str,
    patterns: &'static [&'static str],
    commands: &'static [&'static str],
    version_cmd: &'static [&'static str],
    ports: &'static [&'static str],
    config_hints: &'static [&'static str],
}

#[derive(Default)]
struct MiddlewareHit {
    pids: HashSet<String>,
    units: HashSet<String>,
    ports: HashSet<String>,
    paths: HashSet<String>,
    commands: HashSet<String>,
    unit_states: HashSet<String>,
    version: String,
}

fn specs() -> Vec<MiddlewareSpec> {
    vec![
        MiddlewareSpec {
            id: "nginx",
            name: "Nginx",
            patterns: &["nginx"],
            commands: &["nginx"],
            version_cmd: &["nginx", "-v"],
            ports: &["80", "443"],
            config_hints: &["/etc/nginx/nginx.conf", "/usr/local/nginx/conf/nginx.conf"],
        },
        MiddlewareSpec {
            id: "tomcat",
            name: "Tomcat",
            patterns: &[
                "tomcat",
                "catalina",
                "org.apache.catalina.startup.bootstrap",
            ],
            commands: &["catalina.sh", "startup.sh"],
            version_cmd: &["catalina.sh", "version"],
            ports: &["8080", "8009", "8005"],
            config_hints: &[
                "/etc/tomcat/server.xml",
                "/usr/local/tomcat/conf/server.xml",
                "/opt/tomcat/conf/server.xml",
            ],
        },
        MiddlewareSpec {
            id: "redis",
            name: "Redis",
            patterns: &["redis-server", "redis sentinel"],
            commands: &["redis-server", "redis-cli"],
            version_cmd: &["redis-server", "--version"],
            ports: &["6379", "26379"],
            config_hints: &[
                "/etc/redis/redis.conf",
                "/etc/redis.conf",
                "/usr/local/redis/redis.conf",
            ],
        },
        MiddlewareSpec {
            id: "mysql",
            name: "MySQL/MariaDB",
            patterns: &["mysqld", "mariadbd", "mysql.server"],
            commands: &["mysqld", "mysql", "mariadbd"],
            version_cmd: &["mysql", "--version"],
            ports: &["3306"],
            config_hints: &[
                "/etc/my.cnf",
                "/etc/mysql/my.cnf",
                "/etc/mysql/mysql.conf.d/mysqld.cnf",
            ],
        },
        MiddlewareSpec {
            id: "kafka",
            name: "Kafka",
            patterns: &["kafka", "kafka.kafka", "kafka.server.kafka"],
            commands: &["kafka-server-start.sh", "kafka-topics.sh"],
            version_cmd: &["kafka-topics.sh", "--version"],
            ports: &["9092", "9093", "9094"],
            config_hints: &[
                "/etc/kafka/server.properties",
                "/opt/kafka/config/server.properties",
                "/usr/local/kafka/config/server.properties",
            ],
        },
        MiddlewareSpec {
            id: "elasticsearch",
            name: "Elasticsearch",
            patterns: &["elasticsearch", "org.elasticsearch.bootstrap"],
            commands: &["elasticsearch"],
            version_cmd: &["elasticsearch", "--version"],
            ports: &["9200", "9300"],
            config_hints: &[
                "/etc/elasticsearch/elasticsearch.yml",
                "/usr/local/elasticsearch/config/elasticsearch.yml",
                "/opt/elasticsearch/config/elasticsearch.yml",
            ],
        },
        MiddlewareSpec {
            id: "keepalived",
            name: "Keepalived",
            patterns: &["keepalived"],
            commands: &["keepalived"],
            version_cmd: &["keepalived", "--version"],
            ports: &["112"],
            config_hints: &["/etc/keepalived/keepalived.conf"],
        },
        MiddlewareSpec {
            id: "rabbitmq",
            name: "RabbitMQ",
            patterns: &["rabbitmq", "beam.smp"],
            commands: &["rabbitmqctl", "rabbitmq-server"],
            version_cmd: &["rabbitmqctl", "version"],
            ports: &["5672", "15672", "25672"],
            config_hints: &[
                "/etc/rabbitmq/rabbitmq.conf",
                "/etc/rabbitmq/advanced.config",
            ],
        },
        MiddlewareSpec {
            id: "mongodb",
            name: "MongoDB",
            patterns: &["mongod", "mongodb"],
            commands: &["mongod", "mongo", "mongosh"],
            version_cmd: &["mongod", "--version"],
            ports: &["27017"],
            config_hints: &["/etc/mongod.conf", "/etc/mongodb.conf"],
        },
        MiddlewareSpec {
            id: "postgresql",
            name: "PostgreSQL",
            patterns: &["postgres", "postmaster"],
            commands: &["postgres", "psql"],
            version_cmd: &["psql", "--version"],
            ports: &["5432"],
            config_hints: &["/etc/postgresql", "/var/lib/pgsql/data/postgresql.conf"],
        },
        MiddlewareSpec {
            id: "zookeeper",
            name: "ZooKeeper",
            patterns: &["zookeeper", "quorumpeermain"],
            commands: &["zkServer.sh"],
            version_cmd: &["zkServer.sh", "version"],
            ports: &["2181", "2888", "3888"],
            config_hints: &[
                "/etc/zookeeper/zoo.cfg",
                "/opt/zookeeper/conf/zoo.cfg",
                "/usr/local/zookeeper/conf/zoo.cfg",
            ],
        },
        MiddlewareSpec {
            id: "docker",
            name: "Docker/Containerd",
            patterns: &["dockerd", "containerd"],
            commands: &["docker", "containerd", "dockerd"],
            version_cmd: &["docker", "--version"],
            ports: &[],
            config_hints: &["/etc/docker/daemon.json", "/etc/containerd/config.toml"],
        },
        MiddlewareSpec {
            id: "haproxy",
            name: "HAProxy",
            patterns: &["haproxy"],
            commands: &["haproxy"],
            version_cmd: &["haproxy", "-v"],
            ports: &["80", "443"],
            config_hints: &["/etc/haproxy/haproxy.cfg"],
        },
    ]
}

fn split_cmd_tokens(cmd: &str) -> Vec<String> {
    cmd.split_whitespace()
        .map(|v| v.trim_matches('"').trim_matches('\'').to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn add_existing_path(paths: &mut HashSet<String>, value: &str) {
    let cleaned = value.trim().trim_matches('"').trim_matches('\'');
    if cleaned.is_empty() || cleaned == "-" {
        return;
    }
    if cleaned.starts_with('/') {
        paths.insert(cleaned.to_string());
    }
}

fn extract_paths_from_cmd(cmd: &str, paths: &mut HashSet<String>) {
    let tokens = split_cmd_tokens(cmd);
    let flags = [
        "-c",
        "--config",
        "--conf",
        "--defaults-file",
        "--defaults-extra-file",
        "-f",
        "--pid-file",
    ];
    let prefixes = [
        "--config=",
        "--conf=",
        "--config.file=",
        "--defaults-file=",
        "--defaults-extra-file=",
        "-Dspring.config.location=",
        "--spring.config.location=",
        "-Dlogging.file.name=",
        "-Dlogging.file.path=",
        "-Dpath.conf=",
        "-Dpath.logs=",
        "-Dcatalina.base=",
        "-Dcatalina.home=",
        "-Djava.io.tmpdir=",
        "-jar=",
    ];
    for (idx, token) in tokens.iter().enumerate() {
        for prefix in prefixes {
            if let Some(value) = token.strip_prefix(prefix) {
                add_existing_path(paths, value.trim_start_matches("file:"));
            }
        }
        if flags.contains(&token.as_str()) {
            if let Some(next) = tokens.get(idx + 1) {
                add_existing_path(paths, next);
            }
        }
        if token.starts_with('/')
            && (token.contains("/conf")
                || token.ends_with(".conf")
                || token.ends_with(".yml")
                || token.ends_with(".yaml")
                || token.ends_with(".properties")
                || token.ends_with(".xml")
                || token.ends_with(".jar"))
        {
            add_existing_path(paths, token.trim_start_matches("file:"));
        }
    }
}

fn process_rows() -> Vec<(u32, String, String)> {
    let Some(out) = crate::checks::common::shell_output("ps -eo pid=,comm=,args=") else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse::<u32>().ok()?;
            let comm = parts.next()?.to_string();
            let cmd = parts.collect::<Vec<_>>().join(" ");
            Some((pid, comm, cmd))
        })
        .collect()
}

fn collect_units() -> Vec<(String, String)> {
    let Some(out) = crate::checks::common::shell_output(
        "systemctl list-units --type=service --all --no-legend --no-pager 2>/dev/null",
    ) else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let unit = parts.next()?.to_string();
            let load = parts.next().unwrap_or("-");
            let active = parts.next().unwrap_or("-");
            let sub = parts.next().unwrap_or("-");
            Some((unit, format!("{}/{}/{}", load, active, sub)))
        })
        .collect()
}

fn collect_ports() -> Vec<(String, String)> {
    let Some(out) =
        crate::checks::common::shell_output("ss -ltnup 2>/dev/null || netstat -ltnup 2>/dev/null")
    else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            let local = cols.get(4).or_else(|| cols.get(3))?;
            let port = local.rsplit(':').next()?.trim();
            if port.is_empty() || port == "*" || port == "0" {
                return None;
            }
            Some((port.to_string(), line.to_lowercase()))
        })
        .collect()
}

fn command_version(spec: &MiddlewareSpec) -> String {
    if spec.version_cmd.is_empty() {
        return String::new();
    }
    let cmd = spec.version_cmd[0];
    if crate::checks::common::find_command(cmd).is_none() {
        return String::new();
    }
    let args: Vec<&str> = spec.version_cmd.iter().skip(1).copied().collect();
    crate::checks::common::command_output_timeout(cmd, &args, std::time::Duration::from_secs(2))
        .map(|out| crate::checks::common::truncate(out.lines().next().unwrap_or(""), 140))
        .unwrap_or_default()
}

fn matches_spec(spec: &MiddlewareSpec, text: &str) -> bool {
    let lower = text.to_lowercase();
    spec.patterns.iter().any(|p| lower.contains(p)) || lower.contains(spec.id)
}

fn sorted_join(values: &HashSet<String>, fallback: &str, limit: usize) -> String {
    if values.is_empty() {
        return fallback.to_string();
    }
    let mut list: Vec<String> = values.iter().cloned().collect();
    list.sort();
    if list.len() > limit {
        let extra = list.len() - limit;
        list.truncate(limit);
        list.push(format!("+{}", extra));
    }
    list.join(", ")
}

fn config_hint(spec: &MiddlewareSpec, paths: &HashSet<String>) -> String {
    if !paths.is_empty() {
        return sorted_join(paths, "-", 4);
    }
    let existing: Vec<String> = spec
        .config_hints
        .iter()
        .filter_map(|p| {
            let path = PathBuf::from(p);
            if path.exists() {
                Some((*p).to_string())
            } else {
                None
            }
        })
        .collect();
    if existing.is_empty() {
        "-".to_string()
    } else {
        existing.join(", ")
    }
}

pub fn check() -> CheckResult {
    let all_specs = specs();
    let mut hits: HashMap<&'static str, MiddlewareHit> = all_specs
        .iter()
        .map(|s| (s.id, MiddlewareHit::default()))
        .collect();
    let processes = process_rows();
    let units = collect_units();
    let ports = collect_ports();

    for spec in &all_specs {
        let hit = hits.get_mut(spec.id).expect("middleware hit exists");
        for cmd in spec.commands {
            if let Some(path) = crate::checks::common::find_command(cmd) {
                hit.commands.insert(path);
            }
        }
        hit.version = command_version(spec);
        for hint in spec.config_hints {
            if PathBuf::from(hint).exists() {
                hit.paths.insert((*hint).to_string());
            }
        }
    }

    for (pid, comm, cmd) in &processes {
        let text = format!("{} {}", comm, cmd);
        for spec in &all_specs {
            if !matches_spec(spec, &text) {
                continue;
            }
            let hit = hits.get_mut(spec.id).expect("middleware hit exists");
            hit.pids.insert(pid.to_string());
            extract_paths_from_cmd(cmd, &mut hit.paths);
            if let Ok(path) = std::fs::read_link(format!("/proc/{}/exe", pid)) {
                hit.paths.insert(path.display().to_string());
            }
        }
    }

    for (unit, state) in &units {
        for spec in &all_specs {
            if !matches_spec(spec, unit) {
                continue;
            }
            let hit = hits.get_mut(spec.id).expect("middleware hit exists");
            hit.units.insert(unit.clone());
            hit.unit_states.insert(state.clone());
        }
    }

    for (port, line) in &ports {
        for spec in &all_specs {
            let by_name = matches_spec(spec, line);
            let by_known_port = spec.ports.iter().any(|p| p == port) && !spec.ports.is_empty();
            if by_name
                || (by_known_port
                    && !hits
                        .get(spec.id)
                        .map(|h| h.pids.is_empty() && h.units.is_empty() && h.commands.is_empty())
                        .unwrap_or(true))
            {
                hits.get_mut(spec.id)
                    .expect("middleware hit exists")
                    .ports
                    .insert(port.clone());
            }
        }
    }

    let mut rows = Vec::new();
    let mut labels = Vec::new();
    let mut warn_count = 0usize;
    let mut running_count = 0usize;

    for spec in &all_specs {
        let hit = hits.remove(spec.id).unwrap_or_default();
        let installed =
            !hit.commands.is_empty() || !hit.units.is_empty() || !hit.version.is_empty();
        let running =
            !hit.pids.is_empty() || hit.unit_states.iter().any(|s| s.contains("active/running"));
        if !installed && !running {
            continue;
        }
        if running {
            running_count += 1;
        } else {
            warn_count += 1;
        }
        let status = if running {
            "运行中"
        } else {
            "已安装未运行"
        };
        labels.push(Item::Label {
            key: spec.name.to_string(),
            value: status.to_string(),
            status: Some(if running { "ok" } else { "warn" }.to_string()),
        });
        rows.push(vec![
            spec.name.to_string(),
            status.to_string(),
            sorted_join(&hit.pids, "-", 8),
            sorted_join(&hit.ports, "-", 8),
            sorted_join(&hit.units, "-", 5),
            sorted_join(&hit.unit_states, "-", 4),
            if hit.version.is_empty() {
                "-".to_string()
            } else {
                hit.version
            },
            config_hint(spec, &hit.paths),
            sorted_join(&hit.commands, "-", 3),
        ]);
    }

    let mut sections = vec![Section {
        title: "中间件概览".to_string(),
        icon: Some("MW".to_string()),
        description: Some("基于进程、systemd、监听端口、可执行文件和常见配置路径综合识别；未安装且无运行痕迹的中间件不会显示。".to_string()),
        items: if labels.is_empty() { vec![Item::Info { text: "未检测到常用中间件运行或安装痕迹".to_string() }] } else { labels },
    }];

    if !rows.is_empty() {
        sections.push(Section {
            title: "中间件明细".to_string(),
            icon: Some("LIST".to_string()),
            description: Some(format!(
                "检测到 {} 类中间件，运行中 {} 类，需关注 {} 类",
                rows.len(),
                running_count,
                warn_count
            )),
            items: vec![Item::Table {
                headers: vec![
                    "组件".to_string(),
                    "状态".to_string(),
                    "PID".to_string(),
                    "监听端口".to_string(),
                    "Systemd".to_string(),
                    "Unit状态".to_string(),
                    "版本".to_string(),
                    "路径/配置".to_string(),
                    "命令".to_string(),
                ],
                rows,
                status: Some(if warn_count > 0 { "warn" } else { "ok" }.to_string()),
            }],
        });
    }

    CheckResult {
        id: "middleware".to_string(),
        name: "中间件检查".to_string(),
        description: "Nginx/Tomcat/Kafka/ES/Redis/Keepalived 等中间件运行状态".to_string(),
        category: "常规检查".to_string(),
        version: "1.1.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: if warn_count > 0 {
            CheckStatus::Warn
        } else {
            CheckStatus::Ok
        },
        sections,
    }
}
