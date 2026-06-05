use crate::cli::util::{format_duration_ms, print_heading, status_label};
use crate::config::Config;
use crate::db::Database;
use anyhow::Result;
use colored::*;

pub fn execute(script_id: &str) -> Result<()> {
    let config = Config::load();
    let db_path = crate::config::db_path(&config);
    let db = Database::open(&db_path);

    let (total, success, failure, avg_dur) = db.get_script_stats(script_id);

    if total == 0 {
        println!();
        println!(
            "  {} {}",
            status_label("warn"),
            format!("脚本 '{}' 暂无执行记录", script_id)
                .bright_white()
                .bold()
        );
        println!();
        return Ok(());
    }

    let success_rate = if total > 0 {
        (success as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let records = db.get_history(Some(script_id), 5);

    print_heading("执行统计", Some(script_id));
    println!("  {}", "-".repeat(50).dimmed());
    println!();
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "脚本 ID:".dimmed(),
        script_id.bright_cyan()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "总执行:".dimmed(),
        total.to_string().bright_white().bold()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "成功:".dimmed(),
        success.to_string().truecolor(52, 211, 153)
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "失败:".dimmed(),
        failure.to_string().truecolor(248, 113, 113)
    );
    println!(
        "  {} {} {:.1}%",
        "-".bright_white(),
        "成功率:".dimmed(),
        success_rate
    );

    if let Some(avg) = avg_dur {
        println!();
        println!(
            "  {} {}",
            "[TIME]".bright_yellow().bold(),
            "耗时统计".bright_white().bold()
        );
        println!(
            "  {} {} {}",
            "-".bright_white(),
            "平均:".dimmed(),
            format_duration_ms(avg as u64).bright_cyan()
        );
    }

    if !records.is_empty() {
        println!();
        println!(
            "  {} {}",
            "[HIST]".bright_yellow().bold(),
            "最近执行".bright_white().bold()
        );
        for r in records.iter() {
            let status = match r.exit_code {
                Some(0) => status_label("ok").to_string(),
                Some(c) => format!("[FAIL:{}]", c).truecolor(248, 113, 113).to_string(),
                None => status_label("running").to_string(),
            };
            let dur = r
                .duration_ms
                .map(|d| format_duration_ms(d))
                .unwrap_or_else(|| "-".to_string());
            println!(
                "    {} {} {} {}",
                status,
                r.timestamp.dimmed(),
                dur.bright_cyan(),
                r.script_name.dimmed()
            );
        }
    }
    println!();

    Ok(())
}
