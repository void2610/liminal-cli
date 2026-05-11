use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    // グローバルオプション
    #[arg(long, global = true)]
    pub base_url: Option<String>,
    #[arg(long, global = true)]
    pub token: Option<String>,
    #[arg(long, global = true)]
    pub json: bool,

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

// 単体テスト
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_グローバルパラメータをパースできる() {
        let cli = Cli::try_parse_from([
            "liminal",
            "health",
            "--base-url",
            "http://127.0.0.1:7610",
            "--token",
            "testtesttoken",
            "--json",
        ])
        .unwrap();

        assert_eq!(cli.base_url.unwrap(), "http://127.0.0.1:7610");
        assert_eq!(cli.token.unwrap(), "testtesttoken");
        assert!(cli.json);
    }
}
