use super::common::*;
use super::*;
use std::path::PathBuf;

pub fn check() -> CheckResult {
    let cfg = load_endpoint_config("nginx");
    let config_path = configured_or_first(
        &cfg.config_path,
        &["/etc/nginx/nginx.conf", "/usr/local/nginx/conf/nginx.conf"],
    );
    let log_path = configured_or_first(
        &cfg.log_path,
        &[
            "/var/log/nginx/error.log",
            "/usr/local/nginx/logs/error.log",
        ],
    );
    let nginx_cmd = find_command("nginx");
    let installed = nginx_cmd.is_some();
    let nginx = nginx_cmd.unwrap_or_else(|| "nginx".to_string());
    let version = if installed {
        command_output(&nginx, &["-v"]).unwrap_or_else(|| "未获取到版本".to_string())
    } else {
        "未安装".to_string()
    };
    let test = if installed {
        command_output(&nginx, &["-t"]).unwrap_or_else(|| "无法执行 nginx -t".to_string())
    } else {
        "未安装，跳过 nginx -t".to_string()
    };
    let dump = if installed {
        command_output(&nginx, &["-T"]).unwrap_or_default()
    } else {
        String::new()
    };
    let running = !process_rows(&["nginx: master", "nginx"]).is_empty();
    let available = installed || running;

    let mut sections = Vec::new();
    sections.push(Section {
        title: "连接与程序信息".to_string(),
        icon: Some("NGINX".to_string()),
        description: Some("根据 nginx 命令、进程、端口和配置上下文推断路径".to_string()),
        items: vec![
            label(
                "运行状态",
                if running {
                    "运行中"
                } else {
                    "未发现运行进程"
                },
                Some(if running { "ok" } else { "warn" }),
            ),
            label(
                "程序路径",
                if installed {
                    nginx
                } else {
                    "未安装".to_string()
                },
                None,
            ),
            label("版本", version, None),
            label(
                "配置检测",
                test.clone(),
                Some(if test.contains("successful") || test.contains("ok") {
                    "ok"
                } else {
                    "warn"
                }),
            ),
            label(
                "配置路径",
                path_text_if_available(config_path.as_ref(), available),
                None,
            ),
            label(
                "错误日志",
                path_text_if_available(log_path.as_ref(), available),
                None,
            ),
        ],
    });
    sections.push(table_section(
        "监听端口",
        vec!["协议", "本地地址", "对端", "进程"],
        listen_rows(&["nginx"], &["80", "443", "8080"]),
        "未发现 Nginx 监听端口",
    ));
    sections.push(nginx_context_section(
        "反向代理配置",
        &dump,
        &["proxy_pass", "upstream", "fastcgi_pass", "uwsgi_pass"],
    ));
    sections.push(nginx_context_section(
        "安全配置",
        &dump,
        &[
            "ssl_protocols",
            "ssl_ciphers",
            "add_header",
            "server_tokens",
            "client_max_body_size",
            "allow ",
            "deny ",
        ],
    ));
    sections.push(if available {
        config_preview_section("完整配置预览", config_path)
    } else {
        unavailable_config_section("完整配置预览", "Nginx")
    });
    sections.push(if available {
        log_section("前 100 条异常日志", log_path, 100)
    } else {
        unavailable_log_section("前 100 条异常日志", "Nginx")
    });
    sections.push(table_section(
        "进程信息",
        vec!["PID", "PPID", "用户", "状态", "CPU", "内存", "命令"],
        process_rows(&["nginx"]),
        "未发现 Nginx 进程",
    ));
    CheckResult {
        id: "nginx".to_string(),
        name: "Nginx 常规检查".to_string(),
        description: "端口/连接/反向代理/安全配置/异常日志/路径".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: status_from_bool(running),
        sections,
    }
}

fn nginx_context_section(title: &str, config: &str, keys: &[&str]) -> Section {
    let rows = config
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let lower = line.to_lowercase();
            keys.iter()
                .any(|k| lower.contains(&k.to_lowercase()))
                .then(|| vec![(i + 1).to_string(), truncate(line.trim(), 260)])
        })
        .take(200)
        .collect();
    table_section(
        title,
        vec!["行", "配置"],
        rows,
        "未在配置上下文中匹配到相关项",
    )
}
fn configured_or_first(value: &str, defaults: &[&str]) -> Option<PathBuf> {
    if !value.trim().is_empty() {
        Some(PathBuf::from(value))
    } else {
        first_existing(defaults)
    }
}
fn path_text(path: Option<&PathBuf>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "未推断到".to_string())
}
fn path_text_if_available(path: Option<&PathBuf>, available: bool) -> String {
    if available {
        path_text(path)
    } else {
        "未检测到程序，跳过路径推断".to_string()
    }
}
