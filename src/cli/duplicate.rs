use crate::cli::util::status_label;
use crate::config::{all_script_dirs, Config};
use crate::script;
use anyhow::Result;
use colored::*;

pub fn execute(source_id: &str, new_id: &str) -> Result<()> {
    let config = Config::load();
    let dirs = all_script_dirs(&config);
    let script = script::find_script(&dirs, source_id)?
        .ok_or_else(|| anyhow::anyhow!("未找到脚本: {}", source_id))?;

    let src_dir = script
        .path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取脚本目录"))?;
    let new_id = new_id.trim().to_string();
    if new_id.is_empty() || new_id.contains('/') || new_id.contains('\\') {
        anyhow::bail!("无效的脚本 ID: {}", new_id);
    }

    let dst_dir = config.user_scripts_dir.join(&new_id);
    if dst_dir.exists() {
        anyhow::bail!("脚本 '{}' 已存在", new_id);
    }

    std::fs::create_dir_all(&dst_dir)?;

    for entry in std::fs::read_dir(src_dir)?.flatten() {
        let src = entry.path();
        let fname = entry.file_name();
        if src.is_file() {
            let dst = dst_dir.join(&fname);
            std::fs::copy(&src, &dst)?;
        }
    }

    let toml_path = dst_dir.join(".dm.toml");
    if toml_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            let updated = content
                .lines()
                .map(|line| {
                    if line.starts_with("name") && line.contains('=') {
                        format!("name = \"{}\"", new_id)
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(&toml_path, updated)?;
        }
    }

    for entry in std::fs::read_dir(&dst_dir)?.flatten() {
        let p = entry.path();
        if p.is_file() {
            let fname = p.file_name().unwrap_or_default().to_string_lossy();
            if fname.starts_with(source_id) && fname.ends_with(".sh") {
                let new_fname = format!("{}.sh", new_id);
                std::fs::rename(&p, dst_dir.join(new_fname))?;
            }
        }
    }

    println!();
    println!(
        "  {} {}",
        status_label("ok"),
        format!("脚本已复制为 '{}'", new_id).bright_white().bold()
    );
    println!(
        "  {} {}",
        "-".bright_white(),
        format!("源: {}", source_id).dimmed()
    );
    println!(
        "  {} {}",
        "-".bright_white(),
        format!("目标: {}", new_id).bright_cyan()
    );
    println!(
        "  {} {}",
        "-".bright_white(),
        format!("路径: {}", dst_dir.display()).dimmed()
    );
    println!();

    Ok(())
}
