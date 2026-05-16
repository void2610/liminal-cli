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
    Commands(CommandsArgs),
    /// コマンドを実行する
    Exec(ExecArgs),
    // Logs,
    // State,
    // Scenarios,
    // Run,
}

impl Command {}

#[derive(Args)]
pub struct CommandsArgs {
    /// 指定したパス prefix に一致するコマンドだけを表示する (case-sensitive)
    #[arg(long, value_name = "PREFIX")]
    pub filter: Option<String>,
}

#[derive(Args)]
pub struct ExecArgs {
    /// 実行するコマンドのパス
    pub path: String,

    /// 引数 (KEY=VALUE 形式、複数指定可)
    #[arg(value_parser = parse_key_val)]
    pub args: Vec<(String, String)>,
}

/// "key=value" を (String, String) にパース
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let (key, value) = s
        .split_once('=')
        .ok_or_else(|| format!("'{s}' は KEY=VALUE 形式である必要があります"))?;

    if key.is_empty() {
        return Err(format!("key が空です: '{s}'"));
    }
    Ok((key.to_string(), value.to_string()))
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
