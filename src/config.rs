use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 脚本目录路径（当前目录或项目目录）
    #[serde(default = "default_scripts_dir")]
    pub scripts_dir: PathBuf,
    /// 用户脚本目录 (~/.dm/scripts)
    #[serde(default = "default_user_scripts_dir")]
    pub user_scripts_dir: PathBuf,
    /// Web 服务端口
    #[serde(default = "default_port")]
    pub port: u16,
    /// Web 服务监听地址
    #[serde(default = "default_bind")]
    pub bind: String,
    /// 界面主题
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 界面语言
    #[serde(default = "default_language")]
    pub language: String,
    /// 日志目录
    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,
    /// 数据目录（数据库、规则覆盖、维护数据等）
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

fn default_scripts_dir() -> PathBuf {
    get_base_dir().join("scripts")
}

fn default_user_scripts_dir() -> PathBuf {
    dirs_or_home().join(".dm").join("scripts")
}

fn default_port() -> u16 {
    3399
}

fn default_bind() -> String {
    "0.0.0.0".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_language() -> String {
    "zh".to_string()
}

fn default_log_dir() -> PathBuf {
    dirs_or_home().join(".dm").join("logs")
}

fn default_data_dir() -> PathBuf {
    dirs_or_home().join(".dm").join("data")
}

impl Default for Config {
    fn default() -> Self {
        let base = get_base_dir();
        let home = dirs_or_home();
        Self {
            scripts_dir: base.join("scripts"),
            user_scripts_dir: home.join(".dm").join("scripts"),
            port: 3399,
            bind: default_bind(),
            theme: default_theme(),
            language: default_language(),
            log_dir: home.join(".dm").join("logs"),
            data_dir: home.join(".dm").join("data"),
        }
    }
}

/// 获取所有脚本搜索路径（去重）
pub fn all_script_dirs(config: &Config) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // 项目/当前目录优先
    dirs.push(config.scripts_dir.clone());
    // 用户目录
    let user = config.user_scripts_dir.clone();
    if user != config.scripts_dir {
        dirs.push(user);
    }
    dirs
}

/// 确保用户目录存在（~/.dm/scripts, ~/.dm/logs）
pub fn ensure_user_dirs(config: &Config) {
    let _ = std::fs::create_dir_all(&config.user_scripts_dir);
    let _ = std::fs::create_dir_all(&config.log_dir);
    let _ = std::fs::create_dir_all(&config.data_dir);
}

pub fn db_path(config: &Config) -> PathBuf {
    let _ = std::fs::create_dir_all(&config.data_dir);
    let path = config.data_dir.join("dm.db");
    let legacy = config.log_dir.join("dm.db");
    if legacy.exists() && !path.exists() {
        let _ = std::fs::copy(&legacy, &path);
    }
    path
}

fn dirs_or_home() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home)
    } else {
        PathBuf::from("/root")
    }
}

/// 获取基础目录
fn get_base_dir() -> PathBuf {
    // 1. DM_HOME 环境变量（最高优先级）
    if let Ok(dm_home) = std::env::var("DM_HOME") {
        let p = PathBuf::from(&dm_home);
        if p.join("scripts").exists() {
            return p;
        }
    }

    // 2. 从可执行文件路径向上查找包含 scripts/ 的目录
    if let Ok(exe_path) = std::env::current_exe() {
        let mut dir = exe_path.parent().map(|p| p.to_path_buf());
        while let Some(d) = dir {
            if d.join("scripts").exists() {
                return d;
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
    }

    // 3. 当前工作目录
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("scripts").exists() {
        return cwd;
    }

    // 4. 兜底
    cwd
}

impl Config {
    pub fn load() -> Self {
        let base = get_base_dir();
        let home = dirs_or_home();
        let dm_home = home.join(".dm");

        // 优先级：~/.dm/config.toml > 项目目录/.dm.toml > 默认值
        let user_config = dm_home.join("config.toml");
        let project_config = base.join(".dm.toml");

        let config_path = if user_config.exists() {
            user_config
        } else if project_config.exists() {
            project_config
        } else {
            return Self::default();
        };

        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "警告: 读取配置文件失败 ({}): {}，使用默认配置",
                    config_path.display(),
                    e
                );
                return Self::default();
            }
        };
        let mut config: Config = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("警告: 解析配置文件失败: {}，使用默认配置", e);
                return Self::default();
            }
        };
        if config.scripts_dir.is_relative() {
            config.scripts_dir = base.join(&config.scripts_dir);
        }
        if config.user_scripts_dir.is_relative() {
            config.user_scripts_dir = dm_home.join(&config.user_scripts_dir);
        }
        if config.log_dir.is_relative() {
            config.log_dir = dm_home.join(&config.log_dir);
        }
        if config.data_dir.is_relative() {
            config.data_dir = dm_home.join(&config.data_dir);
        }
        config
    }

    #[allow(dead_code)]
    pub fn config_path() -> PathBuf {
        let home = dirs_or_home();
        home.join(".dm").join("config.toml")
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).unwrap_or_default();
        std::fs::write(&path, content)
    }
}
