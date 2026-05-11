pub mod cli;
pub mod http;

use std::collections::HashMap;

use anyhow::Result;
use cli::{Cli, Command};
use http::{Client, CommandsBody, DEFAULT_URL, HealthBody};
use ureq::Error;

use crate::http::{ExecuteBody, ExecuteResponse};

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
        Command::Execute(args) => {
            //TODO: 文字列->パラメータ列へのデシリアライズが必要
            // Vec<(String, String)> → HashMap<String, String>
            let args_map: HashMap<String, String> = args.args.into_iter().collect();
            let body: ExecuteBody = ExecuteBody {
                path: args.path,
                args: args_map,
            };
            let res: Result<ExecuteResponse, Error> = client.post_execute("/api/v1/execute", &body);
            println!("{:?}", res);
        }
    }

    Ok(())
}
