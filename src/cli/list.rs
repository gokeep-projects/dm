use crate::cli::util::{category_display, dw, pad, print_heading, print_hint, status_label, trunc};
use crate::config::{all_script_dirs, Config};
use crate::db::Database;
use crate::script;
use anyhow::Result;
use colored::*;
use std::collections::HashMap;

fn load_last_exec_map(db: &Database) -> HashMap<String, (String, Option<i32>)> {
    let records = db.get_history(None, 1000);
    let mut map = HashMap::new();
    for r in records {
        map.entry(r.script_id.clone())
            .or_insert((r.timestamp.clone(), r.exit_code));
    }
    map
}

fn format_last_exec(ts: &str) -> String {
    let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") else {
        return ts.to_string();
    };
    let now = chrono::Local::now().naive_local();
    let diff = now.signed_duration_since(dt);
    if diff.num_seconds() < 60 {
        return "刚刚".to_string();
    }
    if diff.num_minutes() < 60 {
        return format!("{}分钟前", diff.num_minutes());
    }
    if diff.num_hours() < 24 {
        return format!("{}小时前", diff.num_hours());
    }
    format!("{}天前", diff.num_days())
}

pub fn execute(search: Option<&str>, category: Option<&str>) -> Result<()> {
    let config = Config::load();
    let db_path = crate::config::db_path(&config);
    let db = Database::open(&db_path);
    let dirs = all_script_dirs(&config);
    let scripts = script::discover_scripts(&dirs)?;
    let last_exec = load_last_exec_map(&db);

    let filtered: Vec<_> = scripts
        .into_iter()
        .filter(|s| {
            if let Some(q) = search {
                let q = q.to_lowercase();
                s.name.to_lowercase().contains(&q)
                    || s.description.to_lowercase().contains(&q)
                    || s.id.to_lowercase().contains(&q)
                    || s.feature.to_lowercase().contains(&q)
                    || s.category.to_lowercase().contains(&q)
            } else {
                true
            }
        })
        .filter(|s| {
            if let Some(c) = category {
                s.category == *c
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        print_heading("没有找到可用脚本", None);
        if search.is_some() || category.is_some() {
            print_hint("尝试调整搜索条件，或直接运行 dm list 查看所有脚本");
        } else {
            print_hint("请将脚本放置在 ~/.dm/scripts/ 目录下");
        }
        println!();
        return Ok(());
    }

    let col_widths = [16, 14, 18, 24, 8, 14];
    let headers = ["ID", "名称", "分类", "功能", "版本", "最近执行"];

    print_heading("可用脚本列表", Some(&format!("({})", filtered.len())));

    let total_width: usize = col_widths.iter().sum::<usize>() + (col_widths.len() - 1) * 3 + 4;
    println!("  {}", "-".repeat(total_width).dimmed());

    print!("  ");
    for (i, h) in headers.iter().enumerate() {
        print!("{}", pad(h, col_widths[i]).bright_cyan().bold());
        if i < headers.len() - 1 {
            print!(" | ");
        }
    }
    println!();

    println!("  {}", "-".repeat(total_width).dimmed());

    for s in filtered.iter() {
        let v = s
            .metadata
            .as_ref()
            .map(|m| m.version.as_str())
            .unwrap_or("-");
        let version = if v == "-" {
            "-".to_string()
        } else {
            format!("v{}", v)
        };
        let last_exec_display = if let Some((ts, exit_code)) = last_exec.get(&s.id) {
            let time_str = format_last_exec(ts);
            match exit_code {
                Some(0) => format!("OK {}", time_str),
                Some(_) => format!("FAIL {}", time_str),
                None => time_str,
            }
        } else {
            "-".to_string()
        };

        print!("  ");
        print!("{}", pad(&s.id, col_widths[0]).bright_white().bold());
        print!(" | ");
        print!(
            "{}",
            pad(&trunc(&s.name, col_widths[1]), col_widths[1]).white()
        );
        print!(" | ");
        let cat_display = category_display(&s.category);
        let cat_plain_width = dw(&format!(
            "[{}] {}",
            crate::cli::util::category_icon(&s.category),
            s.category
        ));
        let cat_pad = if cat_plain_width < col_widths[2] {
            " ".repeat(col_widths[2] - cat_plain_width)
        } else {
            String::new()
        };
        print!("{}{}", cat_display, cat_pad);
        print!(" | ");
        print!(
            "{}",
            pad(&trunc(&s.feature, col_widths[3]), col_widths[3]).dimmed()
        );
        print!(" | ");
        print!("{}", pad(&version, col_widths[4]).truecolor(52, 211, 153));
        print!(" | ");
        print!(
            "{}",
            pad(&trunc(&last_exec_display, col_widths[5]), col_widths[5])
        );
        println!();
    }

    println!("  {}", "-".repeat(total_width).dimmed());

    println!();
    let total = filtered.len();
    let categories: std::collections::HashSet<&str> =
        filtered.iter().map(|s| s.category.as_str()).collect();
    let with_exec = filtered
        .iter()
        .filter(|s| last_exec.contains_key(&s.id))
        .count();
    println!(
        "  {} 共 {} 个脚本  |  {} 个分类  |  {} 个有执行记录",
        status_label("info"),
        total.to_string().bright_white().bold(),
        categories.len().to_string().bright_white().bold(),
        with_exec.to_string().bright_cyan()
    );
    print_hint(&format!(
        "使用 {} 查看详情，{} 执行，{} 查看历史",
        "dm info <id>".bright_cyan(),
        "dm run <id>".bright_cyan(),
        "dm logs <id>".bright_cyan()
    ));
    println!();

    Ok(())
}
