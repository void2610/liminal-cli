pub mod cli;
pub mod http;

use anyhow::Result;
use cli::{Cli, Command};
use http::{DEFAULT_URL, Health};

pub fn run(cli: Cli) -> Result<()> {
    let url: String;
    match cli.base_url {
        Some(u) => url = u,
        None => url = DEFAULT_URL.to_string(),
    }

    match cli.command {
        Command::Health => {
            // ヘルスチェック
            let health: Health = http::get(url + "/api/v1/health")
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
