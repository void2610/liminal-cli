use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use serde::Deserialize;

const DEFAULT_URL: &str = "http://127.0.0.1:7610";

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    // グローバルオプション
    #[arg(long, global = true)]
    base_url: Option<String>,
    #[arg(long, global = true)]
    json: Option<bool>,

    // サブコマンド
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    // Init,
    Health,
    // Doctor,
    /// コマンド一覧を取得する
    Commands,
    /// コマンドを実行する
    Exec(ExecArgs),
    // Logs,
    // State,
    // Scenarios,
    // Run,
}

#[derive(Args)]
struct ExecArgs {
    /// 実行対象コマンドのパス
    path: String,
    /// コマンドに渡すパラメータ
    params: Option<String>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct Health {
    status: String,
    version: String,
    // mode: String,
    // project_name: String,
    // project_path: String,
    // command_count: u32,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let url: String;
    match cli.base_url {
        Some(u) => url = u,
        None => url = DEFAULT_URL.to_string(),
    }

    match cli.command {
        Command::Health => {
            // ヘルスチェック
            let health: Health = ureq::get(url + "/api/v1/health")
                .call()?
                .body_mut()
                .read_json::<Health>()?;
            println!("{:?}", health);
        }
        Command::Commands => {
            println!("コマンド一覧");
        }
        Command::Exec(args) => match args.params {
            Some(p) => {
                println!("{} にパラメータ{}を渡して実行しました", args.path, p);
            }
            None => {
                println!("{} を実行しました", args.path);
            }
        },
    }

    Ok(())
}
