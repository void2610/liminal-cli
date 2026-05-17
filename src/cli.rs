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
    /// 実行履歴を取得する
    Logs(LogsArgs),
    /// LiminalObservableField のスナップショットを取得する
    State(StateArgs),
    /// シナリオ一覧を取得する
    Scenarios(ScenariosArgs),
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
pub struct LogsArgs {
    /// 取得件数の上限 (SPEC §4.7 既定 20、サーバ側上限 200)
    #[arg(long, value_name = "N")]
    pub limit: Option<u32>,
}

#[derive(Args)]
pub struct StateArgs {
    /// 取得するフィールドのパス。省略時は全件取得
    pub path: Option<String>,
}

#[derive(Args)]
pub struct ScenariosArgs {
    /// 指定したパス prefix に一致するシナリオだけを表示する (case-sensitive)
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

    #[test]
    fn cli_commands_filter_をパースできる() {
        let cli = Cli::try_parse_from(["liminal", "commands", "--filter", "Player/"]).unwrap();
        match cli.command {
            Command::Commands(args) => assert_eq!(args.filter.as_deref(), Some("Player/")),
            _ => panic!("expected Commands"),
        }
    }

    #[test]
    fn cli_commands_filter_省略可() {
        let cli = Cli::try_parse_from(["liminal", "commands"]).unwrap();
        match cli.command {
            Command::Commands(args) => assert_eq!(args.filter, None),
            _ => panic!("expected Commands"),
        }
    }

    #[test]
    fn cli_state_path_をパースできる() {
        let cli = Cli::try_parse_from(["liminal", "state", "Player/Health"]).unwrap();
        match cli.command {
            Command::State(args) => assert_eq!(args.path.as_deref(), Some("Player/Health")),
            _ => panic!("expected State"),
        }
    }

    #[test]
    fn cli_state_path_省略可() {
        let cli = Cli::try_parse_from(["liminal", "state"]).unwrap();
        match cli.command {
            Command::State(args) => assert_eq!(args.path, None),
            _ => panic!("expected State"),
        }
    }

    #[test]
    fn cli_logs_limit_をパースできる() {
        let cli = Cli::try_parse_from(["liminal", "logs", "--limit", "10"]).unwrap();
        match cli.command {
            Command::Logs(args) => assert_eq!(args.limit, Some(10)),
            _ => panic!("expected Logs"),
        }
    }

    #[test]
    fn cli_logs_limit_省略可() {
        let cli = Cli::try_parse_from(["liminal", "logs"]).unwrap();
        match cli.command {
            Command::Logs(args) => assert_eq!(args.limit, None),
            _ => panic!("expected Logs"),
        }
    }

    #[test]
    fn cli_scenarios_filter_をパースできる() {
        let cli = Cli::try_parse_from(["liminal", "scenarios", "--filter", "Combat/"]).unwrap();
        match cli.command {
            Command::Scenarios(args) => assert_eq!(args.filter.as_deref(), Some("Combat/")),
            _ => panic!("expected Scenarios"),
        }
    }

    #[test]
    fn cli_exec_key_value_を複数パースできる() {
        let cli = Cli::try_parse_from(["liminal", "exec", "Foo/Bar", "x=1", "y=2"]).unwrap();
        match cli.command {
            Command::Exec(args) => {
                assert_eq!(args.path, "Foo/Bar");
                assert_eq!(
                    args.args,
                    vec![
                        ("x".to_string(), "1".to_string()),
                        ("y".to_string(), "2".to_string()),
                    ]
                );
            }
            _ => panic!("expected Exec"),
        }
    }

    #[test]
    fn cli_exec_value_側に_等号_が含まれてもよい() {
        // split_once('=') なので最初の = だけで分割される
        let cli = Cli::try_parse_from(["liminal", "exec", "Foo", "expr=a=b"]).unwrap();
        match cli.command {
            Command::Exec(args) => {
                assert_eq!(
                    args.args,
                    vec![("expr".to_string(), "a=b".to_string())]
                );
            }
            _ => panic!("expected Exec"),
        }
    }

    #[test]
    fn cli_exec_key_等号無しはエラー() {
        let r = Cli::try_parse_from(["liminal", "exec", "Foo", "novalue"]);
        assert!(r.is_err());
    }

    #[test]
    fn cli_サブコマンド未指定はエラー() {
        let r = Cli::try_parse_from(["liminal"]);
        assert!(r.is_err());
    }
}
