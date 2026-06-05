use crate::cli::util::{print_heading, print_hint, status_label};
use crate::cli::MaintenanceAction;
use crate::maintenance;
use anyhow::Result;
use colored::*;

pub fn execute(action: MaintenanceAction) -> Result<()> {
    match action {
        MaintenanceAction::List { category } => list_records(category.as_deref()),
        MaintenanceAction::Create {
            title,
            description,
            category,
            operator,
        } => create_record(&title, &description, &category, &operator),
        MaintenanceAction::Complete { record_id, result } => complete_record(&record_id, &result),
    }
}

fn list_records(category: Option<&str>) -> Result<()> {
    let records = maintenance::list_records(category);

    if records.is_empty() {
        println!();
        println!(
            "  {} {}",
            status_label("info"),
            "暂无维护记录".bright_white().bold()
        );
        print_hint(&format!(
            "使用 {} 创建新记录",
            "dm maintenance create --title <标题> --description <描述>".bright_cyan()
        ));
        println!();
        return Ok(());
    }

    print_heading("维护记录", Some(&format!("({})", records.len())));
    println!("  {}", "-".repeat(60).dimmed());

    for (i, r) in records.iter().enumerate().take(20) {
        let status = match r.status.as_str() {
            "open" => "[OPEN]".yellow(),
            "completed" => status_label("ok"),
            _ => "[..]".dimmed(),
        };
        let cat = match r.category.as_str() {
            "常规维护" => "[MAINT]",
            "紧急修复" => "[HOTFIX]",
            "系统升级" => "[UPGRADE]",
            "数据迁移" => "[DATA]",
            _ => "[NOTE]",
        };
        println!(
            "  {} {} {} {}",
            format!("{}.", i + 1).dimmed(),
            status,
            format!("{} {}", cat, r.title).bright_white().bold(),
            format!("[{}]", r.category).dimmed()
        );
        if !r.description.is_empty() {
            println!("      {}", r.description.dimmed());
        }
        println!(
            "      {} {} | {} {}",
            "操作人:".dimmed(),
            r.operator.dimmed(),
            "时间:".dimmed(),
            r.timestamp.dimmed()
        );
        if r.status == "completed" && !r.result.is_empty() {
            println!("      {} {}", "结果:".dimmed(), r.result.dimmed());
        }
    }

    println!();
    print_hint(&format!(
        "使用 {} 创建新记录",
        "dm maintenance create".bright_cyan()
    ));
    println!();

    Ok(())
}

fn create_record(title: &str, description: &str, category: &str, operator: &str) -> Result<()> {
    match maintenance::create_record(title, description, category, operator) {
        Ok(record) => {
            println!();
            println!(
                "  {} {}",
                status_label("ok"),
                "维护记录已创建".bright_white().bold()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "ID:".dimmed(),
                record.id.bright_cyan()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "标题:".dimmed(),
                record.title.bright_white()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "分类:".dimmed(),
                record.category.dimmed()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "操作人:".dimmed(),
                record.operator.dimmed()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "时间:".dimmed(),
                record.timestamp.dimmed()
            );
            println!();
            Ok(())
        }
        Err(e) => anyhow::bail!("创建失败: {}", e),
    }
}

fn complete_record(record_id: &str, result: &str) -> Result<()> {
    match maintenance::complete_record(record_id, result) {
        Ok(()) => {
            println!();
            println!(
                "  {} {}",
                status_label("ok"),
                "维护记录已更新为完成".bright_white().bold()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "ID:".dimmed(),
                record_id.bright_cyan()
            );
            println!(
                "  {} {} {}",
                "-".bright_white(),
                "结果:".dimmed(),
                result.dimmed()
            );
            println!();
            Ok(())
        }
        Err(e) => anyhow::bail!("更新失败: {}", e),
    }
}
