use crate::cli::util::{pad, print_heading, print_hint, status_label};
use crate::cli::DocsAction;
use crate::docs;
use anyhow::Result;
use colored::*;
use std::path::Path;

pub fn execute(action: DocsAction) -> Result<()> {
    match action {
        DocsAction::List { category } => list_docs(category.as_deref()),
        DocsAction::Info { doc_id } => show_doc(&doc_id),
        DocsAction::Create {
            doc_id,
            title,
            category,
        } => create_doc(&doc_id, &title, &category),
        DocsAction::Mkdir { name } => mkdir_doc_dir(&name),
        DocsAction::Import {
            file,
            id,
            title,
            category,
        } => import_doc(&file, id.as_deref(), title.as_deref(), &category),
        DocsAction::Update {
            doc_id,
            title,
            category,
            content,
            file,
        } => update_doc(
            &doc_id,
            title.as_deref(),
            category.as_deref(),
            content.as_deref(),
            file.as_deref(),
        ),
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

fn mkdir_doc_dir(name: &str) -> Result<()> {
    let dirs = docs::create_doc_dir(name).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        format!("文档目录 '{}' 已创建", name).bright_white().bold()
    );
    println!(
        "  {} {}",
        "-".bright_white(),
        format!("当前目录: {}", dirs.join(" / ")).dimmed()
    );
    println!();
    Ok(())
}

fn import_doc(file: &Path, id: Option<&str>, title: Option<&str>, category: &str) -> Result<()> {
    let content =
        std::fs::read_to_string(file).map_err(|e| anyhow::anyhow!("读取导入文件失败: {}", e))?;
    if content.trim().is_empty() {
        return Err(anyhow::anyhow!("导入文件内容为空"));
    }
    let filename = file
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("imported-doc");
    let parsed_title = parse_title(&content)
        .or_else(|| title.map(|v| v.to_string()))
        .unwrap_or_else(|| filename.to_string());
    let doc_title = title.unwrap_or(&parsed_title);
    let doc_id = id
        .map(|v| v.to_string())
        .unwrap_or_else(|| slug(&format!("{}-{}", filename, doc_title)));
    let body = strip_import_body(&content);
    let meta = if docs::get_doc(&doc_id).is_some() {
        docs::update_doc(&doc_id, Some(doc_title), Some(category), None, Some(&body))
    } else {
        docs::create_doc(&doc_id, doc_title, category, &body)
    }
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        "文档已导入".bright_white().bold()
    );
    print_doc_meta(&meta);
    println!();
    Ok(())
}

fn update_doc(
    id: &str,
    title: Option<&str>,
    category: Option<&str>,
    content: Option<&str>,
    file: Option<&Path>,
) -> Result<()> {
    let file_content = match file {
        Some(path) => Some(
            std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("读取正文文件失败: {}", e))?,
        ),
        None => None,
    };
    let body = file_content.as_deref().or(content);
    if title.is_none() && category.is_none() && body.is_none() {
        return Err(anyhow::anyhow!(
            "没有需要更新的内容，请指定 --title、--category、--content 或 --file"
        ));
    }
    let meta =
        docs::update_doc(id, title, category, None, body).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        "文档已更新".bright_white().bold()
    );
    print_doc_meta(&meta);
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

fn print_doc_meta(meta: &docs::DocMeta) {
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
        "目录:".dimmed(),
        meta.category.dimmed()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "更新:".dimmed(),
        meta.updated_at.dimmed()
    );
}

fn parse_title(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        line.strip_prefix("# ")
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
    })
}

fn strip_import_body(content: &str) -> String {
    let mut skipped_title = false;
    content
        .lines()
        .filter(|line| {
            if line.starts_with("<!-- ") {
                return false;
            }
            if !skipped_title && line.starts_with("# ") {
                skipped_title = true;
                return false;
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn slug(value: &str) -> String {
    let slug: String = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect();
    let clean = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if clean.is_empty() {
        format!("doc-{}", chrono::Local::now().timestamp())
    } else {
        clean.chars().take(96).collect()
    }
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
