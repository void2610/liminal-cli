pub mod cli;
pub mod http;

use anyhow::Result;
use cli::{Cli, Command};
use http::{Client, CommandsBody, DEFAULT_URL, HealthBody};

pub fn run(cli: Cli) -> Result<()> {
    let url: String;
    let token: String;
    match cli.base_url {
        Some(u) => url = u,
        None => url = DEFAULT_URL.to_string(),
    }
    match cli.token {
        Some(t) => token = t,
        None => token = String::new(),
    }

    let client: Client = Client::new(url).with_token(token);

    match cli.command {
        Command::Health => {
            // ヘルスチェック
            let health: HealthBody = client
                .get("/api/v1/health")
                .call()?
                .body_mut()
                .read_json::<HealthBody>()?;
            println!("{:?}", health);
        }
        Command::Commands => {
            // コマンド一覧
            let commands: CommandsBody = client
                .get("/api/v1/commands")
                .call()?
                .body_mut()
                .read_json::<CommandsBody>()?;
            println!("{:?}", commands);
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
