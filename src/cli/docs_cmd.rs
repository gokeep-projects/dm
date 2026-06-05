use crate::cli::util::{pad, print_heading, print_hint, status_label};
use crate::cli::DocsAction;
use crate::docs;
use anyhow::Result;
use colored::*;

pub fn execute(action: DocsAction) -> Result<()> {
    match action {
        DocsAction::List { category } => list_docs(category.as_deref()),
        DocsAction::Info { doc_id } => show_doc(&doc_id),
        DocsAction::Create {
            doc_id,
            title,
            category,
        } => create_doc(&doc_id, &title, &category),
        DocsAction::Delete { doc_id } => delete_doc(&doc_id),
    }
}

fn list_docs(category: Option<&str>) -> Result<()> {
    let docs = docs::list_docs(category);

    if docs.is_empty() {
        println!();
        println!(
            "  {} {}",
            status_label("warn"),
            "没有找到文档".bright_white().bold()
        );
        print_hint(&format!(
            "使用 {} 创建新文档",
            "dm docs create <id> --title <标题>".bright_cyan()
        ));
        println!();
        return Ok(());
    }

    let cols = [16, 24, 14, 24, 10];
    let headers = ["ID", "标题", "分类", "更新时间", "大小"];

    print_heading("维护文档列表", Some(&format!("({})", docs.len())));
    println!(
        "  {}",
        "-".repeat(cols.iter().sum::<usize>() + cols.len() * 3 + 1)
            .dimmed()
    );

    print!("  {}", "+".dimmed());
    for (i, w) in cols.iter().enumerate() {
        print!("{}", "-".repeat(w + 2).dimmed());
        if i < cols.len() - 1 {
            print!("{}", "+".dimmed());
        }
    }
    println!("{}", "+".dimmed());

    print!("  {}", "|".dimmed());
    for (i, h) in headers.iter().enumerate() {
        print!(" {} {}", pad(h, cols[i]).bright_cyan().bold(), "|".dimmed());
    }
    println!();

    print!("  {}", "+".dimmed());
    for (i, w) in cols.iter().enumerate() {
        print!("{}", "-".repeat(w + 2).dimmed());
        if i < cols.len() - 1 {
            print!("{}", "+".dimmed());
        }
    }
    println!("{}", "+".dimmed());

    for d in &docs {
        let size = format_bytes(d.size_bytes);
        print!("  {}", "|".dimmed());
        print!(
            " {} {}",
            pad(&d.id, cols[0]).bright_white().bold(),
            "|".dimmed()
        );
        print!(" {} {}", pad(&d.title, cols[1]).white(), "|".dimmed());
        print!(" {} {}", pad(&d.category, cols[2]).dimmed(), "|".dimmed());
        print!(" {} {}", pad(&d.updated_at, cols[3]).dimmed(), "|".dimmed());
        print!(" {} {}", pad(&size, cols[4]).dimmed(), "|".dimmed());
        println!();
    }

    print!("  {}", "+".dimmed());
    for (i, w) in cols.iter().enumerate() {
        print!("{}", "-".repeat(w + 2).dimmed());
        if i < cols.len() - 1 {
            print!("{}", "+".dimmed());
        }
    }
    println!("{}", "+".dimmed());

    println!();
    print_hint(&format!(
        "使用 {} 查看文档内容",
        "dm docs info <id>".bright_cyan()
    ));
    println!();

    Ok(())
}

fn show_doc(id: &str) -> Result<()> {
    let doc = docs::get_doc(id).ok_or_else(|| anyhow::anyhow!("未找到文档: {}", id))?;

    print_heading(&doc.meta.title, Some(&format!("[{}]", doc.meta.id)));
    println!("  {}", "-".repeat(60).dimmed());
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "分类:".dimmed(),
        doc.meta.category.bright_cyan()
    );
    if !doc.meta.tags.is_empty() {
        println!(
            "  {} {} {}",
            "-".bright_white(),
            "标签:".dimmed(),
            doc.meta.tags.join(", ").dimmed()
        );
    }
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "创建:".dimmed(),
        doc.meta.created_at.dimmed()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "更新:".dimmed(),
        doc.meta.updated_at.dimmed()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "大小:".dimmed(),
        format_bytes(doc.meta.size_bytes).dimmed()
    );
    println!("  {}", "-".repeat(60).dimmed());
    println!();

    for line in doc.content.lines() {
        if line.starts_with("<!-- ") && line.ends_with(" -->") {
            continue;
        }
        println!("  {}", line);
    }

    println!();
    Ok(())
}

fn create_doc(id: &str, title: &str, category: &str) -> Result<()> {
    let meta = docs::create_doc(id, title, category, "").map_err(|e| anyhow::anyhow!("{}", e))?;

    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        "文档已创建".bright_white().bold()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "ID:".dimmed(),
        meta.id.bright_cyan()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "标题:".dimmed(),
        meta.title.bright_white()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "分类:".dimmed(),
        meta.category.dimmed()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "路径:".dimmed(),
        format!("~/.dm/docs/{}.md", meta.id).dimmed()
    );
    println!();
    Ok(())
}

fn delete_doc(id: &str) -> Result<()> {
    docs::delete_doc(id).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        format!("文档 '{}' 已删除", id).bright_white().bold()
    );
    println!();
    Ok(())
}

fn format_bytes(b: u64) -> String {
    if b < 1024 {
        return format!("{} B", b);
    }
    if b < 1048576 {
        return format!("{:.1} KB", b as f64 / 1024.0);
    }
    format!("{:.1} MB", b as f64 / 1048576.0)
}
