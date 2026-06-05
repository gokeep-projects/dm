use crate::cli::util::status_label;
use crate::config::Config;
use crate::db::Database;
use anyhow::Result;
use colored::*;

pub fn execute() -> Result<()> {
    let config = Config::load();
    let db_path = crate::config::db_path(&config);
    let db = Database::open(&db_path);

    let (total, _, _) = db.get_stats();

    if total == 0 {
        println!();
        println!(
            "  {} {}",
            status_label("info"),
            "执行历史为空，无需清理".dimmed()
        );
        println!();
        return Ok(());
    }

    db.clear_history();

    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        "执行历史已清空".bright_white().bold()
    );
    println!(
        "  {} {}",
        "-".bright_white(),
        format!("已清除 {} 条记录", total).dimmed()
    );
    println!();

    Ok(())
}
