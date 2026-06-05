//! CLI 共享工具函数
//!
//! 提供字符宽度计算、字符串填充/截断、分类图标/颜色映射等
//! 各 CLI 子命令共用的辅助函数。

use colored::*;

/// 计算单个字符的显示宽度
///
/// - ASCII 字符宽度为 1
/// - 表格边框字符 (U+2500 - U+259F) 宽度为 1
/// - CJK 统一表意文字、CJK 标点、全角 ASCII 宽度为 2
/// - 其他字符宽度为 1
pub fn cw(c: char) -> usize {
    if c.is_ascii() {
        return 1;
    }
    let cp = c as u32;
    if (0x2500..=0x259F).contains(&cp) {
        return 1;
    }
    if (0x4E00..=0x9FFF).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0xFF00..=0xFFEF).contains(&cp)
    {
        return 2;
    }
    1
}

/// 计算字符串的显示宽度
pub fn dw(s: &str) -> usize {
    s.chars().map(cw).sum()
}

/// 在右侧填充空格至指定宽度
///
/// 如果字符串超过指定宽度，则截断并附加 `..`
pub fn pad(s: &str, w: usize) -> String {
    let d = dw(s);
    if d > w {
        let mut r = String::new();
        let mut tw = 0;
        for c in s.chars() {
            let cwc = cw(c);
            if tw + cwc > w.saturating_sub(2) {
                r += "..";
                tw += 2;
                break;
            }
            r.push(c);
            tw += cwc;
        }
        if tw < w {
            r += &" ".repeat(w - tw);
        }
        r
    } else {
        format!("{}{}", s, " ".repeat(w - d))
    }
}

/// 截断字符串到指定最大宽度，超出部分以 `..` 表示
pub fn trunc(s: &str, max: usize) -> String {
    let (mut r, mut w) = (String::new(), 0usize);
    for c in s.chars() {
        let cwc = cw(c);
        if w + cwc > max.saturating_sub(2) {
            r += "..";
            break;
        }
        r.push(c);
        w += cwc;
    }
    r
}

/// 分类图标
pub fn category_icon(cat: &str) -> &'static str {
    match cat {
        "系统安全" => "SEC",
        "系统检查" => "SYS",
        "日志管理" => "LOG",
        "服务管理" => "SVC",
        "网络" | "网络诊断" => "NET",
        "中间件" => "MID",
        "Elasticsearch" => "ES",
        "系统管理" => "ADM",
        "性能监控" => "MON",
        _ => "GEN",
    }
}

/// 分类颜色 (RGB truecolor)
pub fn category_color(cat: &str) -> (u8, u8, u8) {
    match cat {
        "系统安全" => (248, 113, 113),
        "系统检查" => (34, 211, 238),
        "日志管理" => (251, 191, 36),
        "服务管理" => (167, 139, 250),
        "网络" => (52, 211, 153),
        "中间件" => (232, 121, 249),
        "Elasticsearch" => (250, 204, 21),
        "系统管理" => (148, 163, 184),
        _ => (209, 213, 219),
    }
}

/// 用分类颜色 + 图标渲染分类字符串
pub fn category_display(cat: &str) -> String {
    let (r, g, b) = category_color(cat);
    format!("[{}] {}", category_icon(cat), cat)
        .truecolor(r, g, b)
        .to_string()
}

/// 稳定宽度的 CLI 状态标签。
pub fn status_label(status: &str) -> ColoredString {
    match status {
        "ok" | "success" | "done" => "[OK]".truecolor(52, 211, 153).bold(),
        "warn" | "warning" => "[WARN]".truecolor(251, 191, 36).bold(),
        "error" | "fail" | "failed" => "[FAIL]".truecolor(248, 113, 113).bold(),
        "running" => "[RUN]".truecolor(34, 211, 238).bold(),
        "info" => "[INFO]".bright_cyan().bold(),
        _ => "[..]".dimmed(),
    }
}

/// 段落标题，避免每个命令手写不一致的标题样式。
pub fn print_heading(title: &str, subtitle: Option<&str>) {
    println!();
    match subtitle {
        Some(s) if !s.is_empty() => println!(
            "  {} {} {}",
            "[DM]".bright_cyan().bold(),
            title.bright_white().bold(),
            s.dimmed()
        ),
        _ => println!(
            "  {} {}",
            "[DM]".bright_cyan().bold(),
            title.bright_white().bold()
        ),
    }
}

pub fn print_hint(text: &str) {
    println!("  {} {}", "[TIP]".cyan().bold(), text);
}

/// 格式化毫秒为人类可读时长
pub fn format_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        return format!("{}ms", ms);
    }
    let s = ms / 1000;
    if s < 60 {
        return format!("{:.2}s", ms as f64 / 1000.0);
    }
    if s < 3600 {
        return format!("{}m{}s", s / 60, s % 60);
    }
    format!("{}h{}m", s / 3600, (s % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_badges_are_ascii_width_stable() {
        assert_eq!(category_icon("系统安全"), "SEC");
        assert_eq!(category_icon("服务管理"), "SVC");
        assert_eq!(category_icon("未知分类"), "GEN");
        assert!(dw("[SYS] 系统检查") <= 18);
    }

    #[test]
    fn truncate_respects_display_width_for_cjk() {
        let s = trunc("系统安全策略检查", 8);
        assert!(dw(&s) <= 8, "{s} is wider than 8 columns");
        assert!(s.ends_with(".."));
    }
}
