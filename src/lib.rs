pub mod cli;
pub mod discovery;
pub mod error;
pub mod http;
pub mod render;
pub mod style;
pub mod token;

use std::collections::HashMap;

use anyhow::Result;
use cli::{Cli, Command};
use discovery::detect_project;
use http::{Client, CommandsResponse, DEFAULT_URL, HealthResponse};
use render::{render_commands, render_health};
use token::get_token;

use crate::{
    http::{
        ExecRequest, ExecResponse, LogsResponse, ScenariosResponse, StateList, StateValue,
        percent_encode,
    },
    render::{render_exec, render_logs, render_scenarios, render_state_list, render_state_value},
};

pub fn run(cli: Cli) -> Result<()> {
    let url: String;
    match cli.base_url {
        Some(u) => url = u,
        None => url = DEFAULT_URL.to_string(),
    }

    // token が None なら認証なしの Client (health 等は SPEC §6 で認証不要)
    let client: Client = match get_token(cli.token) {
        Some(t) => Client::new(url.clone()).with_token(t),
        None => Client::new(url.clone()),
    };

    match cli.command {
        Command::Health => {
            // ヘルスチェック
            let h: HealthResponse = client.get_response::<HealthResponse>("/api/v1/health")?;
            // プロジェクト探索 (cwd 取得失敗は致命にせず未検出扱い)
            let dir = std::env::current_dir()
                .ok()
                .and_then(|cwd| detect_project(&cwd));

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
            let res: ExecResponse = client.post_exec("/api/v1/execute", &body)?;
            render_exec(&res, cli.json)?;
        }
        Command::Logs(args) => {
            // SPEC §4.7: --limit 未指定なら ?limit クエリは付けない
            let endpoint: String = match args.limit {
                Some(n) => format!("/api/v1/logs?limit={}", n),
                None => "/api/v1/logs".to_string(),
            };
            let res: LogsResponse = client.get_response::<LogsResponse>(&endpoint)?;
            render_logs(&res, cli.json)?;
        }
        Command::State(args) => {
            // SPEC §4.8: PATH 指定時は単一フィールド、未指定なら全件
            match args.path {
                Some(path) => {
                    let endpoint = format!("/api/v1/state?path={}", percent_encode(&path));
                    let res: StateValue = client.get_response::<StateValue>(&endpoint)?;
                    render_state_value(&res, cli.json)?;
                }
                None => {
                    let res: StateList = client.get_response::<StateList>("/api/v1/state")?;
                    render_state_list(&res, cli.json)?;
                }
            }
        }
        Command::Scenarios(args) => {
            let mut res: ScenariosResponse =
                client.get_response::<ScenariosResponse>("/api/v1/scenarios")?;
            // --filter 指定時は path prefix が一致するもののみ残す (commands と同じ規約)
            if let Some(filter) = args.filter {
                res.scenarios.retain(|s| s.path.starts_with(&filter));
            }
            render_scenarios(&res, cli.json)?;
        }
    }

    Ok(())
}
