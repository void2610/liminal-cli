use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    // Init,
    // Health,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
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
}
