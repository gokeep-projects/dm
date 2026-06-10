use crate::cli::util::{format_duration_ms, print_heading, print_hint, status_label, trunc};
use crate::java_analyzer::{analyze_java, list_java_processes, JavaAnalysis, JavaAnalyzeRequest};
use anyhow::Result;
use clap::ValueEnum;
use colored::*;
use std::path::PathBuf;
use std::time::Instant;
use tabled::{
    builder::Builder,
    settings::{object::Rows, Alignment, Modify, Style},
};

#[derive(Clone, Debug, ValueEnum)]
pub enum JavaExportFormat {
    Json,
    Raw,
    Report,
    Pdf,
}

pub fn list(search: Option<&str>, json: bool) -> Result<()> {
    let processes = list_java_processes(search);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "processes": processes }))?
        );
        return Ok(());
    }

    print_heading("Java 运行时进程", Some("runtime stack"));
    if processes.is_empty() {
        println!("  {} 未发现运行中的 Java 进程", status_label("warn"));
        print_hint("确认目标进程由 java/javaw/jsvc 启动，或使用 Web 页面顶部过滤框重新扫描");
        println!();
        return Ok(());
    }

    let mut builder = Builder::default();
    builder.push_record(["PID", "服务名", "端口", "线程", "内存", "Attach", "路径"]);
    for p in &processes {
        builder.push_record([
            p.pid.to_string(),
            trunc(&p.service_name, 24),
            if p.ports.is_empty() {
                "-".into()
            } else {
                trunc(&p.ports.join(","), 18)
            },
            p.threads.to_string(),
            fmt_bytes(p.memory_bytes),
            if p.attach_available {
                "ready".into()
            } else {
                "pending".into()
            },
            trunc(if p.cwd.is_empty() { &p.exe } else { &p.cwd }, 44),
        ]);
    }
    let mut table = builder.build();
    table
        .with(Style::rounded())
        .with(Modify::new(Rows::first()).with(Alignment::center()));
    println!("{table}");
    print_hint("使用 dm java analyze --pid <PID> 快速分析，或 dm java export --pid <PID> --format raw 导出原始数据");
    println!();
    Ok(())
}

pub fn analyze(pid: u32, samples: u8, interval_ms: u64, histogram: bool, json: bool) -> Result<()> {
    let started = Instant::now();
    let analysis = analyze_java(JavaAnalyzeRequest {
        pid,
        samples: Some(samples),
        interval_ms: Some(interval_ms),
        include_histogram: Some(histogram),
        cancel_key: None,
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&analysis)?);
        return Ok(());
    }

    render_analysis(&analysis, started.elapsed().as_millis() as u64);
    Ok(())
}

pub fn export(
    pid: u32,
    samples: u8,
    interval_ms: u64,
    histogram: bool,
    format: JavaExportFormat,
    output: PathBuf,
) -> Result<()> {
    let analysis = analyze_java(JavaAnalyzeRequest {
        pid,
        samples: Some(samples),
        interval_ms: Some(interval_ms),
        include_histogram: Some(histogram),
        cancel_key: None,
    })?;
    let content = match format {
        JavaExportFormat::Json => serde_json::to_string_pretty(&analysis)?.into_bytes(),
        JavaExportFormat::Raw => serde_json::to_string_pretty(&serde_json::json!({
            "exported_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            "pid": analysis.process.pid,
            "service_name": analysis.process.service_name,
            "thread_dump": analysis.thread_dump,
            "heap_info": analysis.heap_info,
            "class_histogram": analysis.class_histogram,
            "runtime_logs": analysis.runtime_logs,
            "samples": analysis.samples,
        }))?
        .into_bytes(),
        JavaExportFormat::Report => render_report_text(&analysis).into_bytes(),
        JavaExportFormat::Pdf => render_report_pdf(&analysis),
    };
    std::fs::write(&output, content)?;
    print_heading(
        "Java 分析导出",
        Some(&format!("{:?}", format).to_lowercase()),
    );
    println!(
        "  {} PID {} 已写入 {}",
        status_label("ok"),
        pid.to_string().bright_cyan(),
        output.display().to_string().bright_white()
    );
    println!();
    Ok(())
}

fn render_analysis(analysis: &JavaAnalysis, elapsed_ms: u64) {
    print_heading(
        "Java 堆栈实时分析",
        Some(&format!("PID {}", analysis.process.pid)),
    );
    println!(
        "  {} 服务: {}  端口: {}  Attach: {}  耗时: {}",
        status_label(if analysis.attach_ok { "ok" } else { "warn" }),
        analysis.process.service_name.bright_white().bold(),
        empty_dash(&analysis.process.ports.join(",")).bright_cyan(),
        if analysis.attach_ok {
            "正常".green()
        } else {
            "失败".red()
        },
        format_duration_ms(elapsed_ms).bright_cyan()
    );
    println!(
        "  {} 线程: {}  RUNNABLE: {}  BLOCKED: {}  WAITING: {}  内存: {}",
        status_label("info"),
        analysis.threads.len().to_string().bright_white(),
        summary_value(analysis, "RUNNABLE").bright_cyan(),
        summary_value(analysis, "BLOCKED").yellow(),
        summary_value(analysis, "WAITING").bright_magenta(),
        fmt_bytes(analysis.process.memory_bytes).bright_cyan()
    );

    render_findings(analysis);
    render_hot_frames(analysis);
    render_objects(analysis);
    render_threads(analysis);
    println!();
}

fn render_findings(analysis: &JavaAnalysis) {
    println!();
    println!(
        "  {} {}",
        "[DIAG]".bright_yellow().bold(),
        "异常综合分析".bright_white().bold()
    );
    if analysis.findings.is_empty() {
        println!("  {} 暂无异常结论", status_label("ok"));
        return;
    }
    for finding in analysis.findings.iter().take(12) {
        println!(
            "  {} {}",
            status_label(&finding.level),
            finding.title.bright_white().bold()
        );
        println!("     {}", finding.detail);
        println!("     {}", finding.suggestion.dimmed());
    }
}

fn render_hot_frames(analysis: &JavaAnalysis) {
    println!();
    println!(
        "  {} {}",
        "[CPU]".bright_cyan().bold(),
        "热点方法 Top 12".bright_white().bold()
    );
    let mut builder = Builder::default();
    builder.push_record(["#", "方法", "类型", "命中", "权重"]);
    for (idx, frame) in analysis.hot_frames.iter().take(12).enumerate() {
        builder.push_record([
            (idx + 1).to_string(),
            trunc(&frame.method, 64),
            frame.category.clone(),
            frame.count.to_string(),
            frame.weight.to_string(),
        ]);
    }
    print_table(builder);
}

fn render_objects(analysis: &JavaAnalysis) {
    let rows = class_histogram_rows(&analysis.class_histogram);
    if rows.is_empty() {
        return;
    }
    println!();
    println!(
        "  {} {}",
        "[HEAP]".bright_green().bold(),
        "对象占用 Top 12".bright_white().bold()
    );
    let mut builder = Builder::default();
    builder.push_record(["#", "类名", "实例", "字节"]);
    for (idx, row) in rows.into_iter().take(12).enumerate() {
        builder.push_record([
            (idx + 1).to_string(),
            trunc(&row.0, 58),
            row.1.to_string(),
            fmt_bytes(row.2),
        ]);
    }
    print_table(builder);
}

fn render_threads(analysis: &JavaAnalysis) {
    println!();
    println!(
        "  {} {}",
        "[THR]".bright_magenta().bold(),
        "线程状态 Top 16".bright_white().bold()
    );
    let mut threads = analysis.threads.clone();
    threads.sort_by(|a, b| {
        b.frames
            .len()
            .cmp(&a.frames.len())
            .then_with(|| a.state.cmp(&b.state))
    });
    let mut builder = Builder::default();
    builder.push_record(["状态", "栈深", "线程", "顶部调用"]);
    for thread in threads.into_iter().take(16) {
        builder.push_record([
            thread.state,
            thread.frames.len().to_string(),
            trunc(&thread.name, 28),
            trunc(&thread.top_frame, 58),
        ]);
    }
    print_table(builder);
}

fn render_report_text(analysis: &JavaAnalysis) -> String {
    let mut out = String::new();
    out.push_str("# DM Java 运行时分析报告\n\n");
    out.push_str(&format!(
        "- 导出时间: {}\n- PID: {}\n- 服务名: {}\n- 命令: {}\n- Attach: {}\n- 线程: {}\n- 热点方法: {}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        analysis.process.pid,
        analysis.process.service_name,
        analysis.process.cmd,
        if analysis.attach_ok { "正常" } else { "失败" },
        analysis.threads.len(),
        analysis.hot_frames.len()
    ));
    out.push_str("## 异常综合分析\n\n");
    for finding in &analysis.findings {
        out.push_str(&format!(
            "- [{}] {}\n  - 说明: {}\n  - 建议: {}\n",
            finding.level, finding.title, finding.detail, finding.suggestion
        ));
    }
    out.push_str("\n## CPU 热点\n\n");
    for frame in analysis.hot_frames.iter().take(30) {
        out.push_str(&format!(
            "- {} / {} / 命中 {} / 权重 {}\n",
            frame.method, frame.category, frame.count, frame.weight
        ));
    }
    out.push_str("\n## 对象占用\n\n");
    for (class_name, instances, bytes) in class_histogram_rows(&analysis.class_histogram)
        .into_iter()
        .take(30)
    {
        out.push_str(&format!(
            "- {} / {} / {}\n",
            class_name,
            instances,
            fmt_bytes(bytes)
        ));
    }
    out.push_str("\n## 线程摘要\n\n");
    for thread in analysis.threads.iter().take(40) {
        out.push_str(&format!(
            "- {} / 栈深 {} / {} / {}\n",
            thread.state,
            thread.frames.len(),
            thread.name,
            thread.top_frame
        ));
    }
    out
}

fn render_report_pdf(analysis: &JavaAnalysis) -> Vec<u8> {
    let mut lines = Vec::new();
    lines.push("DM Java 运行时分析报告".to_string());
    lines.push(format!(
        "导出时间: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!("PID: {}", analysis.process.pid));
    lines.push(format!("服务名: {}", analysis.process.service_name));
    lines.push(format!(
        "监听端口: {}",
        empty_dash(&analysis.process.ports.join(","))
    ));
    lines.push(format!("命令: {}", analysis.process.cmd));
    lines.push(format!(
        "Attach: {}",
        if analysis.attach_ok {
            "正常"
        } else {
            "失败"
        }
    ));
    lines.push(format!(
        "线程: {}  热点方法: {}  进程内存: {}",
        analysis.threads.len(),
        analysis.hot_frames.len(),
        fmt_bytes(analysis.process.memory_bytes)
    ));
    if !analysis.attach_error.trim().is_empty() {
        lines.push(format!("Attach 错误: {}", analysis.attach_error));
    }
    lines.push(String::new());

    lines.push("异常综合分析".to_string());
    if analysis.findings.is_empty() {
        lines.push("  暂无异常结论".to_string());
    } else {
        for finding in &analysis.findings {
            lines.push(format!("[{}] {}", finding.level, finding.title));
            lines.push(format!("  说明: {}", finding.detail));
            lines.push(format!("  建议: {}", finding.suggestion));
        }
    }
    lines.push(String::new());

    lines.push("CPU 热点 Top 30".to_string());
    for (idx, frame) in analysis.hot_frames.iter().take(30).enumerate() {
        lines.push(format!(
            "{:02}. {} | {} | 命中 {} | 权重 {}",
            idx + 1,
            frame.method,
            frame.category,
            frame.count,
            frame.weight
        ));
    }
    lines.push(String::new());

    lines.push("对象占用 Top 30".to_string());
    for (idx, (class_name, instances, bytes)) in class_histogram_rows(&analysis.class_histogram)
        .into_iter()
        .take(30)
        .enumerate()
    {
        lines.push(format!(
            "{:02}. {} | 实例 {} | {}",
            idx + 1,
            class_name,
            instances,
            fmt_bytes(bytes)
        ));
    }
    lines.push(String::new());

    lines.push("线程摘要 Top 40".to_string());
    let mut threads = analysis.threads.clone();
    threads.sort_by(|a, b| {
        b.frames
            .len()
            .cmp(&a.frames.len())
            .then_with(|| a.state.cmp(&b.state))
    });
    for thread in threads.into_iter().take(40) {
        lines.push(format!(
            "{} | 栈深 {} | {} | {}",
            thread.state,
            thread.frames.len(),
            thread.name,
            thread.top_frame
        ));
    }
    lines.push(String::new());

    lines.push("堆概要摘录".to_string());
    for line in analysis.heap_info.lines().take(18) {
        lines.push(format!("  {}", line));
    }
    lines.push(String::new());

    lines.push("运行日志".to_string());
    for log in &analysis.runtime_logs {
        lines.push(format!(
            "{} [{}] {} {}%",
            log.timestamp, log.level, log.message, log.percent
        ));
    }

    build_cjk_pdf(&wrap_pdf_lines(&lines, 88))
}

fn wrap_pdf_lines(lines: &[String], max_width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut width = 0usize;
        for ch in line.chars() {
            let ch_width = if ch.is_ascii() { 1 } else { 2 };
            if width + ch_width > max_width && !current.is_empty() {
                out.push(current);
                current = "  ".to_string();
                width = 2;
            }
            current.push(ch);
            width += ch_width;
        }
        out.push(current);
    }
    out
}

fn build_cjk_pdf(lines: &[String]) -> Vec<u8> {
    let lines_per_page = 43usize;
    let pages = lines
        .chunks(lines_per_page)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    let page_count = pages.len().max(1);
    let kids = (0..page_count)
        .map(|idx| format!("{} 0 R", 4 + idx * 2))
        .collect::<Vec<_>>()
        .join(" ");

    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids, page_count).into_bytes());
    objects.push(
        b"<< /Type /Font /Subtype /Type0 /BaseFont /STSong-Light /Encoding /UniGB-UCS2-H /DescendantFonts [<< /Type /Font /Subtype /CIDFontType0 /BaseFont /STSong-Light /CIDSystemInfo << /Registry (Adobe) /Ordering (GB1) /Supplement 5 >> /FontDescriptor << /Type /FontDescriptor /FontName /STSong-Light /Flags 4 /FontBBox [0 -200 1000 900] /ItalicAngle 0 /Ascent 880 /Descent -120 /CapHeight 700 /StemV 80 >> >>] >>".to_vec(),
    );

    for (idx, page_lines) in pages.iter().enumerate() {
        let page_obj = 4 + idx * 2;
        let content_obj = page_obj + 1;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                content_obj
            )
            .into_bytes(),
        );

        let mut stream = String::new();
        for (line_idx, line) in page_lines.iter().enumerate() {
            let font_size = if idx == 0 && line_idx == 0 { 16 } else { 10 };
            let y = 800 - (line_idx as i32 * 17);
            stream.push_str("BT\n");
            stream.push_str(&format!("/F1 {} Tf\n", font_size));
            stream.push_str(&format!("50 {} Td\n", y));
            stream.push('<');
            stream.push_str(&utf16be_hex(line));
            stream.push_str("> Tj\nET\n");
        }
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{}endstream",
                stream.len(),
                stream
            )
            .into_bytes(),
        );
    }

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::with_capacity(objects.len() + 1);
    offsets.push(0usize);
    for (idx, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", idx + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

fn utf16be_hex(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 4);
    for unit in value.encode_utf16() {
        out.push_str(&format!("{:04X}", unit));
    }
    out
}

fn print_table(builder: Builder) {
    let mut table = builder.build();
    table
        .with(Style::rounded())
        .with(Modify::new(Rows::first()).with(Alignment::center()));
    println!("{table}");
}

fn class_histogram_rows(text: &str) -> Vec<(String, u64, u64)> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 || !parts[0].ends_with(':') {
            continue;
        }
        let Ok(instances) = parts[1].parse::<u64>() else {
            continue;
        };
        let Ok(bytes) = parts[2].parse::<u64>() else {
            continue;
        };
        rows.push((parts[3..].join(" "), instances, bytes));
    }
    rows.sort_by(|a, b| b.2.cmp(&a.2));
    rows
}

fn summary_value(analysis: &JavaAnalysis, key: &str) -> String {
    analysis
        .summary
        .get(key)
        .cloned()
        .unwrap_or_else(|| "0".into())
}

fn empty_dash(value: &str) -> String {
    if value.trim().is_empty() {
        "-".to_string()
    } else {
        value.to_string()
    }
}

fn fmt_bytes(value: u64) -> String {
    if value == 0 {
        return "-".into();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value as f64;
    let mut idx = 0usize;
    while size >= 1024.0 && idx < units.len() - 1 {
        size /= 1024.0;
        idx += 1;
    }
    if idx < 2 {
        format!("{:.0} {}", size, units[idx])
    } else {
        format!("{:.1} {}", size, units[idx])
    }
}
