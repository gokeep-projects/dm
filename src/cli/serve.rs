use crate::cli::util::{print_heading, print_hint, status_label};
use crate::config::Config;
use crate::web;
use anyhow::Result;
use colored::*;
use std::fs::OpenOptions;
use std::process::{Command, Stdio};

const DAEMON_CHILD_ENV: &str = "DM_SERVE_DAEMON_CHILD";

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

pub async fn execute(port: u16, bind: &str, daemon: bool) -> Result<()> {
    if daemon && !is_daemon_child() {
        return start_daemon(port, bind);
    }

    run_server(port, bind, !is_daemon_child()).await
}

fn is_daemon_child() -> bool {
    std::env::var_os(DAEMON_CHILD_ENV).is_some()
}

fn start_daemon(port: u16, bind: &str) -> Result<()> {
    let config = Config::load();
    crate::config::ensure_user_dirs(&config);
    let addr = format!("{}:{}", bind, port);

    if let Err(e) = std::net::TcpListener::bind(&addr) {
        anyhow::bail!("端口 {} 已被占用或监听地址不可用: {}", port, e);
    }

    let log_path = config.log_dir.join(format!("dm-serve-{}.log", port));
    let pid_path = config.log_dir.join(format!("dm-serve-{}.pid", port));
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr = stdout.try_clone()?;
    let exe = std::env::current_exe()?;

    let mut command = Command::new(exe);
    command
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--bind")
        .arg(bind)
        .env(DAEMON_CHILD_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    detach_command(&mut command);

    let child = command.spawn()?;
    std::fs::write(&pid_path, format!("{}\n", child.id()))?;

    print_heading("DM Web 服务", Some("后台模式"));
    println!(
        "  {} {}",
        status_label("running"),
        "服务已在后台启动".bright_white().bold()
    );
    println!();
    println!(
        "  {} {} {}",
        "-".cyan(),
        "Web 界面:".dimmed(),
        format!("http://{}", addr).bright_white().bold()
    );
    println!(
        "  {} {} {}",
        "-".yellow(),
        "PID 文件:".dimmed(),
        pid_path.display().to_string().bright_white()
    );
    println!(
        "  {} {} {}",
        "-".magenta(),
        "日志文件:".dimmed(),
        log_path.display().to_string().bright_white()
    );
    println!();
    print_hint(&format!("停止服务: kill {}", child.id()));
    println!();
    Ok(())
}

#[cfg(unix)]
fn detach_command(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn detach_command(_command: &mut Command) {}

async fn run_server(port: u16, bind: &str, open_browser: bool) -> Result<()> {
    let mut config = Config::load();
    config.port = port;
    let addr = format!("{}:{}", bind, port);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!();
            eprintln!(
                "  {} {}",
                status_label("error"),
                format!("无法绑定端口 {}: {}", port, e).bright_red()
            );
            eprintln!();
            eprintln!("  {} 常见解决方案：", "[TIP]".yellow().bold());
            eprintln!(
                "    1. 检查端口占用: {} {}",
                "lsof -i :".dimmed(),
                port.to_string().bright_white()
            );
            eprintln!(
                "    2. 结束占用进程: {}",
                format!("fuser -k {}/tcp", port).bright_white()
            );
            eprintln!(
                "    3. 使用其他端口: {}",
                format!("dm serve --port {}", port + 1).bright_cyan()
            );
            eprintln!();
            anyhow::bail!("端口 {} 已被占用", port);
        }
    };

    print_heading("DM Web 服务", Some("v0.1.0"));
    println!(
        "  {} {}",
        status_label("running"),
        "服务已启动".bright_white().bold()
    );
    println!();
    println!("  {}", "服务信息".bright_white().bold());
    println!("  {}", "-".repeat(50).dimmed());
    println!(
        "  {} {} {}",
        "-".cyan(),
        "Web 界面:".dimmed(),
        format!("http://{}", addr).bright_white().bold()
    );
    println!(
        "  {} {} {}",
        "-".cyan(),
        "监听地址:".dimmed(),
        format!("{}:{}", bind, port).bright_white()
    );
    println!(
        "  {} {} {}",
        "-".yellow(),
        "脚本目录:".dimmed(),
        config.scripts_dir.display().to_string().bright_white()
    );
    println!(
        "  {} {} {}",
        "-".magenta(),
        "用户目录:".dimmed(),
        config.user_scripts_dir.display().to_string().bright_white()
    );

    let sys = crate::dashboard::get_system_info();
    let dirs = crate::config::all_script_dirs(&config);
    let script_count = crate::script::discover_scripts(&dirs)
        .map(|s| s.len())
        .unwrap_or(0);

    println!();
    println!("  {}", "系统信息".bright_white().bold());
    println!("  {}", "-".repeat(50).dimmed());
    println!(
        "  {} {} {}",
        "-".cyan(),
        "CPU:".dimmed(),
        format!("{} 核 ({})", sys.cpu_count, sys.cpu_brand).bright_white()
    );
    println!(
        "  {} {} {}",
        "-".cyan(),
        "内存:".dimmed(),
        format!(
            "{:.1}% ({}/{})",
            sys.memory_usage,
            fmt_bytes(sys.memory_used),
            fmt_bytes(sys.memory_total)
        )
        .bright_white()
    );
    println!(
        "  {} {} {}",
        "-".cyan(),
        "磁盘:".dimmed(),
        format!(
            "{:.1}% ({}/{})",
            sys.disk_usage,
            fmt_bytes(sys.disk_used),
            fmt_bytes(sys.disk_total)
        )
        .bright_white()
    );
    println!(
        "  {} {} {}",
        "-".yellow(),
        "脚本:".dimmed(),
        format!("{} 个", script_count).bright_white()
    );
    println!();
    print_hint("按 Ctrl+C 停止服务");
    println!();
    if open_browser {
        try_open_browser(port);
    }

    let app = web::build_router(config);
    axum::serve(listener, app).await?;
    Ok(())
}

fn try_open_browser(port: u16) {
    let url = format!("http://127.0.0.1:{}", port);
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    match std::process::Command::new(opener)
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => {
            println!(
                "  {} 已尝试打开本地浏览器: {}",
                status_label("info"),
                url.bright_cyan()
            );
            println!();
        }
        Err(_) => {
            println!(
                "  {} 未检测到可用本地浏览器，请手动访问 {}",
                status_label("info"),
                url.bright_cyan()
            );
            println!();
        }
    }
}
