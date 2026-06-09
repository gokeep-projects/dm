pub mod check;
pub mod check_config;
pub mod clean;
pub mod docs_cmd;
pub mod duplicate;
pub mod info;
pub mod list;
pub mod logs;
pub mod maintenance;
pub mod run;
pub mod serve;
pub mod stats;
pub mod util;
pub mod version;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "dm", version, about = "现场维护工具 - 脚本管理与执行平台")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 启动 Web 服务
    Serve {
        /// 监听端口
        #[arg(short, long, default_value = "3399")]
        port: u16,
        /// 监听地址
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
        /// 后台启动服务
        #[arg(short = 'd', long)]
        daemon: bool,
    },
    /// 列出所有可用脚本
    List {
        /// 搜索关键词
        #[arg(short, long)]
        search: Option<String>,
        /// 按分类筛选
        #[arg(short, long)]
        category: Option<String>,
    },
    /// 查看脚本详情
    Info {
        /// 脚本 ID
        script_id: String,
    },
    /// 执行脚本 (默认美化渲染，--json 输出原始 JSON)
    Run {
        /// 脚本 ID
        script_id: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        #[arg(short, long = "param", value_name = "KEY=VALUE")]
        params: Vec<String>,
        #[arg(short, long, default_value = "0")]
        timeout: u64,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 查看脚本执行统计
    Stats {
        /// 脚本 ID
        script_id: String,
    },
    /// 复制脚本
    Duplicate {
        /// 源脚本 ID
        source_id: String,
        /// 新脚本 ID
        new_id: String,
    },
    /// 清空执行历史
    Clean,
    /// 查看脚本执行历史
    Logs {
        /// 脚本 ID
        script_id: String,
    },
    /// 显示版本信息
    Version,
    /// 维护文档管理
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },
    /// 执行常规检查 (默认美化渲染，--json 输出原始 JSON)
    Check {
        /// 检查项 ID
        check_id: String,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 导出全部核心常规检查信息
    CheckExport {
        /// 输出文件路径；不指定时输出到终端
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// 仅输出原始 JSON，不显示摘要
        #[arg(long)]
        json: bool,
    },
    /// 管理常规检查连接配置
    CheckConfig {
        #[command(subcommand)]
        action: CheckConfigAction,
    },
    /// 维护记录管理
    Maintenance {
        #[command(subcommand)]
        action: MaintenanceAction,
    },
    /// 生成 shell 命令补全脚本
    Completions {
        /// 目标 shell: bash/zsh/fish/powershell/elvish
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand)]
pub enum DocsAction {
    /// 列出所有文档
    List {
        /// 按分类筛选
        #[arg(short, long)]
        category: Option<String>,
    },
    /// 查看文档内容
    Info {
        /// 文档 ID
        doc_id: String,
    },
    /// 创建新文档
    Create {
        /// 文档 ID
        doc_id: String,
        /// 文档标题
        #[arg(short, long)]
        title: String,
        /// 文档分类
        #[arg(short, long, default_value = "通用")]
        category: String,
    },
    /// 删除文档
    Delete {
        /// 文档 ID
        doc_id: String,
    },
}

#[derive(Subcommand)]
pub enum MaintenanceAction {
    /// 列出维护记录
    List {
        /// 按分类筛选
        #[arg(short, long)]
        category: Option<String>,
    },
    /// 创建维护记录
    Create {
        /// 标题
        #[arg(short, long)]
        title: String,
        /// 描述
        #[arg(short, long)]
        description: String,
        /// 分类
        #[arg(short, long, default_value = "常规维护")]
        category: String,
        /// 操作人
        #[arg(short, long, default_value = "system")]
        operator: String,
    },
    /// 完成维护记录
    Complete {
        /// 记录 ID
        record_id: String,
        /// 结果
        #[arg(short, long)]
        result: String,
    },
}

#[derive(Subcommand)]
pub enum CheckConfigAction {
    /// 查看检查配置
    Get {
        /// 检查项 ID，例如 elasticsearch/redis/mysql/java-service
        check_id: String,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 保存检查配置，格式: key=value
    Set {
        /// 检查项 ID，例如 elasticsearch/redis/mysql/java-service
        check_id: String,
        /// 配置项，示例 host=127.0.0.1 port=9200 username=elastic
        #[arg(value_name = "KEY=VALUE")]
        values: Vec<String>,
    },
    /// 导入 JSON 检查连接配置
    Import {
        /// JSON 配置文件路径
        file: std::path::PathBuf,
    },
    /// 导出当前检查连接配置
    Export {
        /// 输出文件路径；不指定时输出到终端
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
    /// 输出 JSON 配置模板
    Template {
        /// 输出文件路径；不指定时输出到终端
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn serve_short_daemon_flag_is_parsed() {
        let cli = Cli::try_parse_from(["dm", "serve", "-d", "--port", "3401"]).unwrap();
        match cli.command {
            Some(Commands::Serve { port, bind, daemon }) => {
                assert_eq!(port, 3401);
                assert_eq!(bind, "0.0.0.0");
                assert!(daemon);
            }
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn serve_long_daemon_flag_is_parsed() {
        let cli = Cli::try_parse_from(["dm", "serve", "--daemon", "--bind", "127.0.0.1"]).unwrap();
        match cli.command {
            Some(Commands::Serve { port, bind, daemon }) => {
                assert_eq!(port, 3399);
                assert_eq!(bind, "127.0.0.1");
                assert!(daemon);
            }
            _ => panic!("expected serve command"),
        }
    }
}
