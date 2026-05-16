pub mod cli;
pub mod http;
pub mod render;
pub mod style;
pub mod token;

use std::collections::HashMap;

use anyhow::Result;
use cli::{Cli, Command};
use http::{Client, CommandsBody, DEFAULT_URL, HealthBody};
use render::render_health;
use token::get_token;
use ureq::Error;

use crate::http::{ExecuteBody, ExecuteResponse};

pub fn run(cli: Cli) -> Result<()> {
    let url: String;
    match cli.base_url {
        Some(u) => url = u,
        None => url = DEFAULT_URL.to_string(),
    }

    let token: String = get_token(cli.token).unwrap();
    let client: Client = Client::new(url.clone()).with_token(token);

    match cli.command {
        Command::Health => {
            // ヘルスチェック
            let h: HealthBody = client
                .get("/api/v1/health")
                .call()?
                .body_mut()
                .read_json::<HealthBody>()?;
            render_health(&h, &url, cli.json)?;
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
