use crate::cli::util::{category_color, format_duration_ms, print_heading, status_label};
use crate::config::{all_script_dirs, Config};
use crate::db::Database;
use crate::script;
use crate::script::executor::{resolve_interpreter, system_environment};
use anyhow::Result;
use colored::*;
use std::io::{self, Write};
use std::process::Command;
use std::time::Instant;

pub async fn execute(
    script_id: &str,
    args: &[String],
    params: &[String],
    timeout: u64,
    json_mode: bool,
) -> Result<()> {
    let config = Config::load();
    let db_path = crate::config::db_path(&config);
    let db = Database::open(&db_path);
    let dirs = all_script_dirs(&config);
    let script = script::find_script(&dirs, script_id)?.ok_or_else(|| {
        eprintln!();
        eprintln!(
            "  {} {}",
            status_label("error"),
            format!("未找到脚本: {}", script_id).bright_white().bold()
        );
        eprintln!();
        eprintln!(
            "  {} 使用 {} 查看所有脚本",
            "[TIP]".yellow().bold(),
            "dm list".bright_cyan()
        );
        eprintln!(
            "  {} 使用 {} 查看脚本详情",
            "[TIP]".yellow().bold(),
            "dm info <id>".bright_cyan()
        );
        eprintln!();
        anyhow::anyhow!("未找到脚本: {}", script_id)
    })?;

    let start_time = Instant::now();
    let param_json = params
        .iter()
        .filter_map(|param| param.split_once('='))
        .map(|(key, value)| {
            (
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    db.insert_exec_with_inputs(
        &script.id,
        &script.name,
        None,
        None,
        0,
        &serde_json::Value::Object(param_json),
        args,
    );

    if script.path.starts_with(&config.scripts_dir) {
        if let Some(result) = crate::checks::run_check(script_id) {
            db.update_exec(&script.id, 0, result.duration_ms, 0);
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                crate::cli::check::render_result(&result);
            }
            return Ok(());
        }
    }

    let (cmd, mut cmd_args) = resolve_interpreter(&script.path);

    for arg in args {
        cmd_args.push(arg.clone());
    }

    for param in params {
        if let Some((key, value)) = param.split_once('=') {
            cmd_args.push(format!("--{}={}", key, value));
        } else {
            cmd_args.push(param.clone());
        }
    }

    let (r, g, b) = category_color(&script.category);

    if !json_mode {
        print_heading("执行脚本", Some(&script.id));
        println!("  {}", "-".repeat(50).dimmed());
        println!(
            "  {} {}: {}",
            "-".bright_white(),
            "名称".dimmed(),
            script.name.bright_white()
        );
        println!(
            "  {} {}: {}",
            "-".bright_white(),
            "ID".dimmed(),
            script.id.bright_cyan()
        );
        println!(
            "  {} {}: {}",
            "-".bright_white(),
            "分类".dimmed(),
            script.category.truecolor(r, g, b)
        );
        if let Some(ref m) = script.metadata {
            if !m.version.is_empty() {
                println!(
                    "  {} {}: {}",
                    "-".bright_white(),
                    "版本".dimmed(),
                    format!("v{}", m.version).truecolor(52, 211, 153)
                );
            }
        }
        if !args.is_empty() || !params.is_empty() {
            let arg_str = if args.is_empty() {
                String::new()
            } else {
                args.join(" ")
            };
            let param_str = if params.is_empty() {
                String::new()
            } else {
                format!(
                    " {}",
                    params
                        .iter()
                        .map(|p| {
                            if p.contains('=') {
                                format!("--{}", p)
                            } else {
                                p.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            println!(
                "  {} {}: {}{}",
                "-".bright_white(),
                "参数".dimmed(),
                arg_str.bright_white(),
                param_str.dimmed()
            );
        }
        println!();
        print!(
            "  {} {}",
            status_label("running"),
            "运行中...".bright_white()
        );
        io::stdout().flush().ok();
        println!();
        println!("  {} {}", "-".bright_white(), "命令".dimmed());
        println!(
            "    {}",
            format!("{} {}", cmd, cmd_args.join(" ")).bright_cyan()
        );
        if timeout > 0 {
            println!("  {} {}", "-".bright_white(), "超时".dimmed());
            println!("    {}秒", timeout.to_string().bright_yellow());
        }
        println!("  {}", "-".repeat(50).dimmed());
    }

    if json_mode {
        let output = if timeout > 0 {
            let dur = std::time::Duration::from_secs(timeout);
            match tokio::time::timeout(dur, async {
                Command::new(&cmd)
                    .args(&cmd_args)
                    .envs(system_environment())
                    .output()
            })
            .await
            {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => {
                    let elapsed = start_time.elapsed();
                    let json = serde_json::json!({
                        "script_id": script.id,
                        "script_name": script.name,
                        "exit_code": -1,
                        "elapsed_ms": elapsed.as_millis() as u64,
                        "success": false,
                        "timeout": true,
                        "stdout": "",
                        "stderr": "",
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json).unwrap_or_default()
                    );
                    return Ok(());
                }
            }
        } else {
            Command::new(&cmd)
                .args(&cmd_args)
                .envs(system_environment())
                .output()?
        };
        let elapsed = start_time.elapsed();
        let code = output.status.code().unwrap_or(-1);
        db.update_exec(&script.id, code, elapsed.as_millis() as u64, 0);
        let json = serde_json::json!({
            "script_id": script.id,
            "script_name": script.name,
            "exit_code": code,
            "elapsed_ms": elapsed.as_millis() as u64,
            "success": output.status.success(),
            "timeout": false,
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );
        if !output.status.success() {
            std::process::exit(code);
        }
        return Ok(());
    }

    let status = if timeout > 0 {
        let dur = std::time::Duration::from_secs(timeout);
        match tokio::time::timeout(dur, async {
            Command::new(&cmd)
                .args(&cmd_args)
                .envs(system_environment())
                .status()
        })
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                db.update_exec(&script.id, -1, start_time.elapsed().as_millis() as u64, 0);
                return Err(e.into());
            }
            Err(_) => {
                let elapsed = start_time.elapsed();
                db.update_exec(&script.id, 124, elapsed.as_millis() as u64, 0);
                eprintln!();
                eprintln!(
                    "  {} {} {}",
                    status_label("warning"),
                    "执行超时".bright_red().bold(),
                    format!(
                        "(超时: {}秒, 耗时: {})",
                        timeout,
                        format_duration_ms(elapsed.as_millis() as u64)
                    )
                    .dimmed()
                );
                eprintln!();
                anyhow::bail!("脚本执行超时 ({}秒)", timeout);
            }
        }
    } else {
        Command::new(&cmd)
            .args(&cmd_args)
            .envs(system_environment())
            .status()?
    };
    let elapsed = start_time.elapsed();

    println!("  {}", "-".repeat(50).dimmed());

    let elapsed_str = format_duration_ms(elapsed.as_millis() as u64);

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        db.update_exec(&script.id, code, elapsed.as_millis() as u64, 0);
        eprintln!();
        eprintln!(
            "  {} {} {}",
            status_label("error"),
            "脚本执行失败".bright_red().bold(),
            format!("(退出码: {}, 耗时: {})", code, elapsed_str).dimmed()
        );
        eprintln!();
        eprintln!(
            "  {} 使用 {} 查看完整输出",
            "[TIP]".yellow().bold(),
            "Web 界面".bright_cyan()
        );
        eprintln!();
        anyhow::bail!("脚本执行失败 (退出码: {})", code);
    }

    println!();
    println!(
        "  {} {} {}",
        status_label("success"),
        "执行完成".bright_green().bold(),
        format!("(耗时: {})", elapsed_str).dimmed()
    );
    println!();
    db.update_exec(&script.id, 0, elapsed.as_millis() as u64, 0);

    Ok(())
}
