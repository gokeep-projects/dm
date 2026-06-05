use crate::checks;
use crate::cli::util;
use anyhow::Result;
use colored::*;
use std::path::PathBuf;
use tabled::{
    builder::Builder,
    settings::{object::Rows, Alignment, Modify, Style},
};

const CORE_CHECK_IDS: &[&str] = &[
    "system",
    "resource",
    "service",
    "network",
    "security",
    "middleware",
    "elasticsearch",
    "redis",
    "nginx",
    "keepalived",
    "mysql",
    "java-service",
];

pub fn execute(check_id: &str, json: bool) -> Result<()> {
    let result =
        checks::run_check(check_id).ok_or_else(|| anyhow::anyhow!("未找到检查项: {}", check_id))?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        );
        return Ok(());
    }

    render_result(&result);
    Ok(())
}

pub fn export_all(output: Option<PathBuf>, json_only: bool) -> Result<()> {
    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    let mut results = Vec::new();
    let mut total_warnings = 0usize;
    let mut total_errors = 0usize;
    let mut ok_count = 0usize;
    let mut warn_count = 0usize;
    let mut error_count = 0usize;

    for id in CORE_CHECK_IDS {
        match checks::run_check(id) {
            Some(result) => {
                let (warnings, errors) = count_findings(&result);
                total_warnings += warnings;
                total_errors += errors;
                match &result.status {
                    checks::CheckStatus::Ok | checks::CheckStatus::Info => ok_count += 1,
                    checks::CheckStatus::Warn => warn_count += 1,
                    checks::CheckStatus::Error => error_count += 1,
                }
                rows.push(vec![
                    result.id.clone(),
                    result.name.clone(),
                    status_text(&result.status).to_string(),
                    warnings.to_string(),
                    errors.to_string(),
                    result.duration_ms.to_string(),
                    result.sections.len().to_string(),
                ]);
                results.push(serde_json::json!({
                    "id": result.id,
                    "name": result.name,
                    "description": result.description,
                    "category": result.category,
                    "version": result.version,
                    "timestamp": result.timestamp,
                    "status": result.status,
                    "duration_ms": result.duration_ms,
                    "warning_count": warnings,
                    "error_count": errors,
                    "section_count": result.sections.len(),
                    "sections": result.sections,
                }));
            }
            None => {
                warn_count += 1;
                total_warnings += 1;
                rows.push(vec![
                    (*id).to_string(),
                    (*id).to_string(),
                    "warn".to_string(),
                    "1".to_string(),
                    "0".to_string(),
                    "0".to_string(),
                    "0".to_string(),
                ]);
                results.push(serde_json::json!({
                    "id": id,
                    "name": id,
                    "status": "warn",
                    "duration_ms": 0,
                    "warning_count": 1,
                    "error_count": 0,
                    "sections": [{
                        "title": "检查无结果",
                        "items": [{
                            "type": "warning",
                            "text": "检查项没有返回结构化结果"
                        }]
                    }]
                }));
            }
        }
    }

    let overall = if total_errors > 0 || error_count > 0 {
        "error"
    } else if total_warnings > 0 || warn_count > 0 {
        "warn"
    } else {
        "ok"
    };
    let payload = serde_json::json!({
        "exported_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "duration_ms": started.elapsed().as_millis() as u64,
        "overall_status": overall,
        "total": results.len(),
        "scope": {
            "type": "core_health_checks",
            "check_ids": CORE_CHECK_IDS,
            "note": "导出核心常规检查的完整结构化数据；外部插件请使用 dm check <id> 单独执行"
        },
        "summary": {
            "ok": ok_count,
            "warn": warn_count,
            "error": error_count,
            "warnings": total_warnings,
            "errors": total_errors,
        },
        "checks": results,
    });
    let total = rows.len();
    let pretty = serde_json::to_string_pretty(&payload)?;

    if let Some(path) = output {
        std::fs::write(&path, pretty)?;
        if !json_only {
            render_export_summary(&rows, total, total_warnings, total_errors, overall);
            println!(
                "  {} 已写入 {}",
                util::status_label("ok"),
                path.display().to_string().bright_cyan()
            );
            println!();
        }
        return Ok(());
    }

    if json_only {
        println!("{}", pretty);
    } else {
        render_export_summary(&rows, total, total_warnings, total_errors, overall);
        println!("{}", pretty);
    }
    Ok(())
}

pub fn render_result(result: &checks::CheckResult) {
    util::print_heading(&result.name, Some(&result.category));
    println!("  {} {}", "-".bright_white(), result.description.dimmed());
    println!("  {}", "-".repeat(60).dimmed());
    let (warnings, errors) = count_findings(result);
    println!(
        "  {} 状态: {}  分区: {}  警告: {}  错误: {}  耗时: {}",
        util::status_label(status_text(&result.status)),
        color_status(status_text(&result.status)),
        result.sections.len().to_string().bright_cyan(),
        warnings.to_string().yellow(),
        errors.to_string().red(),
        util::format_duration_ms(result.duration_ms).bright_white()
    );
    println!("  {}", "-".repeat(60).dimmed());

    for section in &result.sections {
        println!();
        println!(
            "  {} {}",
            "[SECTION]".bright_cyan().bold(),
            section.title.bright_white().bold()
        );
        if let Some(desc) = &section.description {
            println!("  {} {}", "-".bright_white(), desc.dimmed());
        }
        println!("  {}", "-".repeat(40).dimmed());

        for item in &section.items {
            render_item(item);
        }
    }

    println!();
    println!("  {}", "-".repeat(60).dimmed());
    println!(
        "  {} 耗时: {}ms",
        util::status_label("info"),
        result.duration_ms
    );
    println!();
}

fn render_export_summary(
    rows: &[Vec<String>],
    total: usize,
    warnings: usize,
    errors: usize,
    overall: &str,
) {
    util::print_heading("全部常规检查导出", Some("core checks"));
    println!(
        "  {} 状态: {}  检查项: {}  警告: {}  错误: {}",
        util::status_label(overall),
        color_status(overall),
        total.to_string().bright_cyan(),
        warnings.to_string().yellow(),
        errors.to_string().red()
    );
    println!();
    render_table(
        &[
            "ID".to_string(),
            "名称".to_string(),
            "状态".to_string(),
            "警告".to_string(),
            "错误".to_string(),
            "耗时ms".to_string(),
            "分区".to_string(),
        ],
        rows,
    );
    println!();
}

fn count_findings(result: &checks::CheckResult) -> (usize, usize) {
    let mut warn = 0usize;
    let mut error = 0usize;
    for section in &result.sections {
        for item in &section.items {
            match item {
                checks::Item::Label {
                    status: Some(s), ..
                }
                | checks::Item::Bar {
                    status: Some(s), ..
                }
                | checks::Item::Table {
                    status: Some(s), ..
                }
                | checks::Item::Sparkline {
                    status: Some(s), ..
                } => {
                    if s == "error" {
                        error += 1;
                    } else if s == "warn" {
                        warn += 1;
                    }
                }
                checks::Item::Warning { .. } => warn += 1,
                checks::Item::Error { .. } => error += 1,
                checks::Item::Finding { level, .. } => {
                    if level == "error" {
                        error += 1;
                    } else if level == "warn" {
                        warn += 1;
                    }
                }
                _ => {}
            }
        }
    }
    (warn, error)
}

fn status_text(status: &checks::CheckStatus) -> &'static str {
    match status {
        checks::CheckStatus::Ok => "ok",
        checks::CheckStatus::Warn => "warn",
        checks::CheckStatus::Error => "error",
        checks::CheckStatus::Info => "info",
    }
}

fn color_status(status: &str) -> colored::ColoredString {
    match status {
        "ok" => "ok".bright_green().bold(),
        "warn" => "warn".bright_yellow().bold(),
        "error" => "error".bright_red().bold(),
        "info" => "info".bright_cyan().bold(),
        _ => status.white(),
    }
}

fn render_item(item: &checks::Item) {
    match item {
        checks::Item::Label { key, value, status } => {
            let status_icon = status_label_from_opt(status.as_deref());
            println!(
                "  {} {}: {}",
                status_icon,
                key.dimmed(),
                value.bright_white()
            );
        }
        checks::Item::Bar {
            key,
            value,
            max,
            unit,
            status,
        } => {
            let pct = if *max > 0.0 {
                (value / max * 100.0).clamp(0.0, 100.0) as u32
            } else {
                0
            };
            let bar_width: usize = 20;
            let filled = (pct as f64 / 100.0 * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(filled);
            let bar = format!("{}{}", "#".repeat(filled), "-".repeat(empty));
            let status_icon = status_label_from_opt(status.as_deref());
            println!(
                "  {} {} [{}] {:.1}{}",
                status_icon,
                key.dimmed(),
                color_bar(&bar, pct),
                value,
                unit
            );
        }
        checks::Item::Table {
            headers,
            rows,
            status: _,
        } => render_table(headers, rows),
        checks::Item::Info { text } => {
            println!("  {} {}", util::status_label("info"), text.white())
        }
        checks::Item::Warning { text } => {
            println!("  {} {}", util::status_label("warn"), text.yellow())
        }
        checks::Item::Error { text } => {
            println!("  {} {}", util::status_label("error"), text.red())
        }
        checks::Item::Success { text } => {
            println!("  {} {}", util::status_label("ok"), text.green())
        }
        checks::Item::Finding {
            rule_id,
            level,
            category,
            title,
            target,
            summary,
            evidence,
            suggestion,
            commands,
        } => {
            let status = util::status_label(level);
            println!(
                "  {} {} {}",
                status,
                title.bright_white().bold(),
                format!("({})", rule_id).dimmed()
            );
            println!("     {}: {}", "对象".dimmed(), target.bright_cyan());
            println!("     {}: {}", "分类".dimmed(), category.bright_white());
            println!("     {}: {}", "概要".dimmed(), summary.bright_white());
            if !evidence.is_empty() {
                println!("     {}", "证据".dimmed());
                for line in evidence {
                    println!("       - {}", line.white());
                }
            }
            if !suggestion.is_empty() {
                println!("     {}: {}", "建议".dimmed(), suggestion.yellow());
            }
            if !commands.is_empty() {
                println!("     {}", "命令".dimmed());
                for cmd in commands {
                    println!("       {}", cmd.bright_cyan());
                }
            }
        }
        checks::Item::Divider => println!("  {}", "-".repeat(40).dimmed()),
        checks::Item::Sparkline {
            key,
            data,
            unit,
            status,
        } => {
            let status_icon = status_label_from_opt(status.as_deref());
            let current = data.last().unwrap_or(&0.0);
            let max = data.iter().cloned().fold(f64::MIN, f64::max);
            println!(
                "  {} {} 当前: {:.1}{} 最大: {:.1}{}",
                status_icon,
                key.dimmed(),
                current,
                unit,
                max,
                unit
            );
        }
    }
}

fn status_label_from_opt(status: Option<&str>) -> colored::ColoredString {
    match status {
        Some("ok") => util::status_label("ok"),
        Some("warn") => util::status_label("warn"),
        Some("error") => util::status_label("error"),
        _ => util::status_label("info"),
    }
}

fn color_bar(bar: &str, pct: u32) -> colored::ColoredString {
    if pct < 60 {
        bar.green()
    } else if pct < 80 {
        bar.yellow()
    } else {
        bar.red()
    }
}

fn render_table(headers: &[String], rows: &[Vec<String>]) {
    if headers.is_empty() {
        return;
    }
    let max_cell_width = if headers.len() >= 6 {
        34
    } else if headers.len() >= 4 {
        48
    } else {
        80
    };
    let mut builder = Builder::default();
    builder.push_record(headers.iter().map(|h| h.to_string()));
    for row in rows {
        let normalized = headers.iter().enumerate().map(|(i, _)| {
            util::trunc(
                row.get(i).map(String::as_str).unwrap_or("-"),
                max_cell_width,
            )
        });
        builder.push_record(normalized);
    }

    let mut table = builder.build();
    table
        .with(Style::ascii_rounded())
        .with(Modify::new(Rows::first()).with(Alignment::center()))
        .with(Modify::new(Rows::new(1..)).with(Alignment::left()));

    for line in table.to_string().lines() {
        println!("  {}", line.white());
    }
}
