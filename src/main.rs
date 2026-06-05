mod anomaly;
mod checks;
mod cli;
mod config;
mod dashboard;
mod db;
mod docs;
mod maintenance;
mod script;
mod web;

use clap::{CommandFactory, Parser};
use cli::util::status_label;
use cli::{CheckConfigAction, Cli, Commands};
use colored::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let cfg = config::Config::load();
    config::ensure_user_dirs(&cfg);

    let result = match cli.command {
        Some(Commands::Serve { port, bind }) => cli::serve::execute(port, &bind).await,
        Some(Commands::List { search, category }) => {
            cli::list::execute(search.as_deref(), category.as_deref())
        }
        Some(Commands::Info { script_id }) => cli::info::execute(&script_id),
        Some(Commands::Run {
            script_id,
            args,
            params,
            timeout,
            json,
        }) => cli::run::execute(&script_id, &args, &params, timeout, json).await,
        Some(Commands::Stats { script_id }) => cli::stats::execute(&script_id),
        Some(Commands::Duplicate { source_id, new_id }) => {
            cli::duplicate::execute(&source_id, &new_id)
        }
        Some(Commands::Clean) => cli::clean::execute(),
        Some(Commands::Logs { script_id }) => cli::logs::execute(&script_id),
        Some(Commands::Version) => cli::version::execute(),
        Some(Commands::Maintenance { action }) => cli::maintenance::execute(action),
        Some(Commands::Docs { action }) => cli::docs_cmd::execute(action),
        Some(Commands::Check { check_id, json }) => cli::check::execute(&check_id, json),
        Some(Commands::CheckExport { output, json }) => cli::check::export_all(output, json),
        Some(Commands::CheckConfig { action }) => match action {
            CheckConfigAction::Get { check_id, json } => cli::check_config::get(&check_id, json),
            CheckConfigAction::Set { check_id, values } => {
                cli::check_config::set(&check_id, &values)
            }
        },
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(())
        }
        None => cli::serve::execute(3399, "0.0.0.0").await,
    };

    if let Err(e) = result {
        eprintln!();
        eprintln!(
            "  {} {}",
            status_label("error"),
            "发生错误".bright_red().bold()
        );
        eprintln!("  {}", "-".repeat(50).dimmed());
        eprintln!(
            "  {} {}",
            "[ERR]".red().bold(),
            e.to_string().bright_white()
        );
        eprintln!("  {}", "-".repeat(50).dimmed());
        eprintln!();
        eprintln!(
            "  {} 使用 {} 查看帮助",
            "[TIP]".yellow().bold(),
            "dm --help".bright_cyan()
        );
        eprintln!();
        std::process::exit(1);
    }
}
