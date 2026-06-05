use crate::cli::util::print_heading;
use anyhow::Result;
use colored::*;

pub fn execute() -> Result<()> {
    print_heading("DM 现场维护工具", None);
    println!("  {}", "-".repeat(40).dimmed());
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "版本:".dimmed(),
        env!("CARGO_PKG_VERSION").bright_cyan()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "描述:".dimmed(),
        env!("CARGO_PKG_DESCRIPTION").dimmed()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "Rust:".dimmed(),
        "stable".bright_white()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "平台:".dimmed(),
        format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH).bright_white()
    );
    println!(
        "  {} {} {}",
        "-".bright_white(),
        "构建:".dimmed(),
        chrono::Local::now().format("%Y-%m-%d").to_string().dimmed()
    );
    println!();
    Ok(())
}
