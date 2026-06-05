use crate::cli::util::{format_duration_ms, print_heading, status_label};
use crate::config::Config;
use crate::db::Database;
use anyhow::Result;
use colored::*;

pub fn execute(script_id: &str) -> Result<()> {
    let config = Config::load();
    let db_path = crate::config::db_path(&config);
    let db = Database::open(&db_path);

    let records = db.get_history(Some(script_id), 20);

    if records.is_empty() {
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

    print_heading("执行历史", Some(script_id));
    println!("  {}", "-".repeat(60).dimmed());
    println!();

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

    println!();
    println!(
        "  {} 共 {} 条记录",
        status_label("info"),
        records.len().to_string().bright_white()
    );
    println!();

    Ok(())
}
