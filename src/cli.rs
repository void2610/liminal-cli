use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    // グローバルオプション
    #[arg(long, global = true)]
    pub base_url: Option<String>,
    #[arg(long, global = true)]
    pub json: Option<bool>,

    // サブコマンド
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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

impl Command {}

#[derive(Args)]
pub(crate) struct ExecArgs {
    /// 実行対象コマンドのパス
    pub path: String,
    /// コマンドに渡すパラメータ
    pub params: Option<String>,
}
