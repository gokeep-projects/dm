use crate::cli::util::{category_color, pad, print_heading, print_hint, status_label};
use crate::config::{all_script_dirs, Config};
use crate::script;
use anyhow::Result;
use colored::*;

pub fn execute(id: &str) -> Result<()> {
    let config = Config::load();
    let dirs = all_script_dirs(&config);
    let script =
        script::find_script(&dirs, id)?.ok_or_else(|| anyhow::anyhow!("未找到脚本: {}", id))?;

    let w = 84;
    let border_thin = "-".repeat(w - 2);
    let (r, g, b) = category_color(&script.category);

    print_heading(&script.name, Some(&format!("({})", script.id)));

    println!();
    println!("  {}", format!("+{}+", border_thin).dimmed());

    let rows: Vec<(&str, &str, String)> = vec![
        ("ID", "标识", script.id.bright_white().bold().to_string()),
        ("NAME", "名称", script.name.bright_white().to_string()),
        (
            "PATH",
            "路径",
            script.path.display().to_string().dimmed().to_string(),
        ),
        (
            "CAT",
            "分类",
            script.category.truecolor(r, g, b).to_string(),
        ),
        ("FEAT", "功能", script.feature.to_string()),
        ("DESC", "描述", script.description.dimmed().to_string()),
    ];

    for (code, label, value) in rows {
        let line = format!("  [{:<4}] {:<4}: {}", code, label, value);
        println!("  {}{}{}", "|".dimmed(), pad(&line, w - 2), "|".dimmed());
    }

    if let Some(ref m) = script.metadata {
        if !m.version.is_empty() {
            let line = format!(
                "  [{:<4}] {:<4}: {}",
                "VER",
                "版本",
                format!("v{}", m.version).truecolor(52, 211, 153)
            );
            println!("  {}{}{}", "|".dimmed(), pad(&line, w - 2), "|".dimmed());
        }
        if !m.author.is_empty() {
            let line = format!(
                "  [{:<4}] {:<4}: {}",
                "AUTH",
                "作者",
                m.author.bright_white()
            );
            println!("  {}{}{}", "|".dimmed(), pad(&line, w - 2), "|".dimmed());
        }
        if let Some(modified) = &script.modified {
            if !modified.is_empty() {
                let line = format!("  [{:<4}] {:<4}: {}", "TIME", "修改", modified.dimmed());
                println!("  {}{}{}", "|".dimmed(), pad(&line, w - 2), "|".dimmed());
            }
        }
    }

    println!("  {}", format!("+{}+", border_thin).dimmed());

    if let Some(ref m) = script.metadata {
        if !m.example.is_empty() || !m.params.is_empty() {
            println!();
            println!();
        }

        if !m.example.is_empty() {
            println!(
                "  {} {}",
                status_label("info"),
                "使用示例".bright_white().bold()
            );
            for line in m.example.lines() {
                println!("    {}", line.bright_white());
            }
            println!();
        }

        if !m.params.is_empty() {
            println!(
                "  {} {}",
                status_label("info"),
                "参数列表".bright_white().bold()
            );
            for p in &m.params {
                let req = if p.required {
                    " *".red().bold().to_string()
                } else {
                    "".to_string()
                };
                let def = p.default.as_deref().unwrap_or("-");
                let required_tag = if p.required {
                    "[必填]".red().to_string()
                } else {
                    "[可选]".dimmed().to_string()
                };
                println!(
                    "    - {}{}: {} {} (默认: {})",
                    p.name.bright_white().bold(),
                    req,
                    p.description.dimmed(),
                    required_tag,
                    def.bright_green()
                );
            }
            println!();
        }
    }

    print_hint(&format!(
        "使用 {} 执行，{} 查看历史，{} 复制脚本",
        format!("dm run {}", id).bright_cyan(),
        format!("dm logs {}", id).bright_cyan(),
        format!("dm duplicate {} <new_id>", id).bright_cyan()
    ));
    println!();

    Ok(())
}
