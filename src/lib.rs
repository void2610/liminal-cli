pub mod cli;
pub mod http;
pub mod render;
pub mod style;
pub mod token;

use std::collections::HashMap;

use anyhow::Result;
use cli::{Cli, Command};
use http::{Client, CommandsResponse, DEFAULT_URL, HealthResponse};
use render::{render_commands, render_health};
use token::get_token;

use crate::{
    http::{ExecRequest, ExecResponse},
    render::render_exec,
};

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
            let h: HealthResponse = client.get_response::<HealthResponse>("/api/v1/health")?;
            render_health(&h, &url, cli.json)?;
        }
        Command::Commands(args) => {
            // コマンド一覧
            let mut commands: CommandsResponse =
                client.get_response::<CommandsResponse>("/api/v1/commands")?;
            // --filter 指定時は path prefix が一致するもののみ残す (case-sensitive、SPEC §4.5)
            if let Some(filter) = args.filter {
                commands.commands.retain(|c| c.path.starts_with(&filter));
            }
            render_commands(&commands, cli.json)?;
        }
        Command::Exec(args) => {
            // Vec<(String, String)> → HashMap<String, String>
            let args_map: HashMap<String, String> = args.args.into_iter().collect();
            let body: ExecRequest = ExecRequest {
                path: args.path,
                args: args_map,
            };
            let res: ExecResponse = client.post_exec("/api/v1/execute", &body).unwrap();
            render_exec(&res, cli.json)?;
        }
    }

    Ok(())
}
