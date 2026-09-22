pub mod cache;
pub mod cli;
pub mod commands;
pub mod discovery;
pub mod error;
pub mod http;
pub mod render;
pub mod style;
pub mod token;

use std::collections::HashMap;

use anyhow::Result;
use cli::{Cli, Command, ModeArg};
use discovery::{DiscoveryOptions, Mode};
use http::Client;
use render::{render_commands, render_health};
use token::get_token;

use serde_json::Value;

use crate::{
    http::{ExecRequest, percent_encode},
    render::{render_exec, render_logs, render_scenarios, render_state_list, render_state_value},
};

/// `{key: [ {path: ...}, ... ]}` の配列を path prefix で絞り込む。
/// `--json` は絞り込み後を出すので、生の Value の段階で落とす (SPEC §4.5 / §4.9)。
fn retain_by_path(v: &mut Value, key: &str, prefix: &str) {
    if let Some(Value::Array(items)) = v.get_mut(key) {
        items.retain(|item| {
            item.get("path")
                .and_then(Value::as_str)
                .is_some_and(|p| p.starts_with(prefix))
        });
    }
}

impl From<ModeArg> for Mode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Editor => Mode::Editor,
            ModeArg::Runtime => Mode::Runtime,
        }
    }
}

/// ポート番号は 1..=65535 (SPEC §3)。u16 では 0 を弾けないのでここで検証する。
fn validate_port(port: Option<u16>, flag: &str) -> Result<()> {
    match port {
        Some(0) => anyhow::bail!("{flag} は 1..=65535 の範囲で指定してください: 0"),
        _ => Ok(()),
    }
}

pub fn run(cli: Cli) -> Result<()> {
    let mode = cli.mode.map(Mode::from);
    validate_port(cli.port, "--port")?;

    // サーバ不要のコマンド (SPEC §4) は discovery を走らせる前に処理する。
    match cli.command {
        Command::Init(args) => return commands::init::run(&args, cli.port, mode),
        Command::Doctor(args) => {
            return commands::doctor::run(&args, cli.project.as_deref(), mode, cli.port);
        }
        Command::Project(sub) => return commands::project::run(&sub, mode),
        _ => {}
    }

    // 以降はサーバが要る。接続先を決めてから Client を組み立てる。
    let opts = DiscoveryOptions {
        base_url: cli.base_url.clone(),
        port: cli.port,
        project: cli.project.clone(),
        mode,
    };
    let resolved = discovery::resolve(&opts)?;
    let url = resolved.base_url.clone();

    // token が None なら認証なしの Client (health 等は SPEC §6 で認証不要)
    let client: Client = match get_token(cli.token) {
        Some(t) => Client::new(url.clone()).with_token(t),
        None => Client::new(url.clone()),
    };

    match cli.command {
        // discovery 済みのコマンドはここには来ない
        Command::Init(_) | Command::Doctor(_) | Command::Project(_) => unreachable!(),
        Command::Health => {
            // discovery 中に取得済みの body があれば使い回す (probe と本リクエストの二度打ちを避ける)
            let v = match resolved.health {
                Some(body) => Value::Object(body),
                None => client.get_value("/api/v1/health")?,
            };
            render_health(&v, &url, cli.json)?;
        }
        Command::Commands(args) => {
            let mut v = client.get_value("/api/v1/commands")?;
            // --filter 指定時は path prefix が一致するもののみ残す (case-sensitive、SPEC §4.5)
            if let Some(filter) = args.filter {
                retain_by_path(&mut v, "commands", &filter);
            }
            render_commands(&v, cli.json)?;
        }
        Command::Exec(args) => {
            // Vec<(String, String)> → HashMap<String, String>
            let args_map: HashMap<String, String> = args.args.into_iter().collect();
            let body = serde_json::to_value(ExecRequest {
                path: args.path,
                args: args_map,
            })?;
            let v = client.post_value("/api/v1/execute", &body)?;
            render_exec(&v, cli.json)?;
        }
        Command::Logs(args) => {
            // SPEC §4.7: --limit 未指定なら ?limit クエリは付けない
            let endpoint: String = match args.limit {
                Some(n) => format!("/api/v1/logs?limit={}", n),
                None => "/api/v1/logs".to_string(),
            };
            let v = client.get_value(&endpoint)?;
            render_logs(&v, cli.json)?;
        }
        Command::State(args) => {
            // SPEC §4.8: PATH 指定時は単一フィールド、未指定なら全件
            match args.path {
                Some(path) => {
                    let endpoint = format!("/api/v1/state?path={}", percent_encode(&path));
                    let v = client.get_value(&endpoint)?;
                    render_state_value(&v, cli.json)?;
                }
                None => {
                    let v = client.get_value("/api/v1/state")?;
                    render_state_list(&v, cli.json)?;
                }
            }
        }
        Command::Scenarios(args) => {
            let mut v = client.get_value("/api/v1/scenarios")?;
            // --filter 指定時は path prefix が一致するもののみ残す (commands と同じ規約)
            if let Some(filter) = args.filter {
                retain_by_path(&mut v, "scenarios", &filter);
            }
            render_scenarios(&v, cli.json)?;
        }
        Command::Run(args) => {
            commands::run::run(&client, &args, cli.json)?;
        }
    }

    Ok(())
}
