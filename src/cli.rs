use clap::{Args, Parser, Subcommand, ValueEnum};

/// `--version` は版だけでなく出自も出す。手元のバイナリがどこの何版か、
/// CLI 自身から辿れないと不具合の報告先が分からなくなるため。
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\n",
    env!("CARGO_PKG_REPOSITORY")
);

#[derive(Parser)]
#[command(version, long_version = LONG_VERSION, about)]
pub struct Cli {
    // グローバルオプション (SPEC §3)
    /// ベース URL を直接指定する (discovery をバイパス)
    #[arg(long, global = true, value_name = "URL")]
    pub base_url: Option<String>,
    /// ポートだけ指定する (隣接探索もしない)
    #[arg(long, global = true, value_name = "N")]
    pub port: Option<u16>,
    /// 対象プロジェクトを名前かパスで指定する
    #[arg(long, global = true, value_name = "NAME_OR_PATH")]
    pub project: Option<String>,
    /// Editor 側か Play Mode 側かを指定する
    #[arg(long, global = true, value_name = "MODE")]
    pub mode: Option<ModeArg>,
    /// Bearer トークン ($LP_TOKEN / ~/.liminal-palette/token より優先)
    #[arg(long, global = true, value_name = "TOKEN")]
    pub token: Option<String>,
    /// JSON を生のまま出力する
    #[arg(long, global = true)]
    pub json: bool,

    // サブコマンド
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// `--mode` の値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ModeArg {
    Editor,
    Runtime,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// プロジェクトの onboarding 状態を表示し、必要なら preferred port を書き込む
    Init(InitArgs),
    /// サーバの生存を確認する
    Health,
    /// 環境を診断する (token / プロジェクト検出 / キャッシュ / 生存ポート)
    Doctor(DoctorArgs),
    /// プロジェクト設定を表示 / 変更する
    #[command(subcommand)]
    Project(ProjectCommand),
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
    /// シナリオを実行する (named / glob / ad-hoc)
    Run(RunArgs),
}

#[derive(Subcommand)]
pub(crate) enum ProjectCommand {
    /// 現在のプロジェクト設定と live listener を表示する
    Show,
    /// preferred port を書き込む
    SetPort(SetPortArgs),
    /// preferred port を削除する
    UnsetPort(UnsetPortArgs),
}

#[derive(Args)]
pub struct InitArgs {
    /// Play Mode 用 preferred port を書き込む
    #[arg(long, value_name = "N")]
    pub runtime_port: Option<u16>,
    // Editor 用の書き込みはグローバルの --port を使う。
    // clap では global 引数と同名をサブコマンド側に定義できないため (`liminal init --port N` は同じ働き)。
}

#[derive(Args)]
pub struct DoctorArgs {
    /// probe に応答しなかったキャッシュエントリを削除する
    #[arg(long)]
    pub prune_stale: bool,
}

#[derive(Args)]
pub struct SetPortArgs {
    /// 書き込むポート (1..=65535)
    // フィールド名がそのまま clap の引数 id になるため、global な --port と衝突しない名前にする。
    #[arg(value_name = "PORT")]
    pub value: u32,
    /// runtimePort 側に書き込む
    #[arg(long)]
    pub runtime: bool,
}

#[derive(Args)]
pub struct UnsetPortArgs {
    /// runtimePort 側を削除する
    #[arg(long)]
    pub runtime: bool,
}

#[derive(Args)]
pub struct RunArgs {
    /// シナリオパス。glob (`*` `?` `[`) を含めると複数実行。--steps 使用時は省略する
    pub path: Option<String>,

    /// ad-hoc ステップを JSON で読み込む (`-` で stdin)
    #[arg(long, value_name = "FILE_OR_DASH")]
    pub steps: Option<String>,

    /// JUnit XML レポートの出力先
    #[arg(long, value_name = "PATH")]
    pub report: Option<String>,
}

impl Command {
    /// Bearer トークンが要るサブコマンドか (SPEC §4 の一覧)。
    /// `health` と、サーバを使わない `init` / `doctor` / `project` は不要。
    pub(crate) fn requires_auth(&self) -> bool {
        matches!(
            self,
            Command::Commands(_)
                | Command::Exec(_)
                | Command::Logs(_)
                | Command::State(_)
                | Command::Scenarios(_)
                | Command::Run(_)
        )
    }
}

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
// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#[allow(non_snake_case)]
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
                assert_eq!(args.args, vec![("expr".to_string(), "a=b".to_string())]);
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
