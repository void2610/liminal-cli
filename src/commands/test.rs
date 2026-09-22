use std::time::{Duration, Instant};

use anstream::println;
use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::cli::{TestArgs, TestMode};
use crate::error::ExecFailure;
use crate::http::Client;
use crate::style::{DIM, GREEN, RED, YELLOW};

const RUN_ENDPOINT: &str = "/api/v1/tests/run";
const RESULT_ENDPOINT: &str = "/api/v1/tests/result";

/// `liminal test`。Unity Test Runner を開始し、完了まで polling して結果を表示する。
/// exit code: 完了して Passed → 0、それ以外 → 2 (running / idle は失敗ではないので 0)。
pub(crate) fn run(client: &Client, args: &TestArgs, json_out: bool) -> Result<()> {
    // result は現在の状態を 1 回取るだけ。
    if args.test_mode == TestMode::Result {
        let body = client.get_value(RESULT_ENDPOINT)?;
        emit_status(&body, json_out)?;
        return exit_for(&body);
    }

    let mode = mode_str(args.test_mode);
    let mut req = json!({ "mode": mode });
    if let Some(f) = &args.filter {
        req["filter"] = Value::from(f.clone());
    }
    if args.force {
        req["force"] = Value::from(true);
    }

    // 既に実行中 (409) なら、待機モードでは進行中のランに相乗りする。
    match client.post_value(RUN_ENDPOINT, &req) {
        Ok(resp) => {
            if !json_out {
                let started = resp
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or(mode)
                    .to_string();
                let filter = resp
                    .get("filter")
                    .and_then(Value::as_str)
                    .unwrap_or("all")
                    .to_string();
                println!("{DIM}started {started} (filter={filter}){DIM:#}");
            }
            if args.no_wait {
                if json_out {
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
                return Ok(());
            }
        }
        Err(e) => {
            if !is_already_running(&e) {
                return Err(e);
            }
            if args.no_wait {
                if !json_out {
                    println!(
                        "{YELLOW}既にテスト実行中です (liminal test result で状況を確認できます){YELLOW:#}"
                    );
                    println!("{DIM}中断された状態が残っている場合は --force で解除できます{DIM:#}");
                }
                return Err(ExecFailure.into());
            }
            if !json_out {
                println!("{YELLOW}既にテスト実行中 → 進行中のランを polling します{YELLOW:#}");
            }
        }
    }

    poll_until_done(client, args, json_out)
}

// 完了まで待つ。PlayMode テストは DomainReload でサーバが一時的に落ちるので、
// 接続エラーはタイムアウトまで握りつぶして再試行する。
fn poll_until_done(client: &Client, args: &TestArgs, json_out: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs_f64(args.timeout);
    let interval = Duration::from_secs_f64(args.interval);

    loop {
        std::thread::sleep(interval);
        match client.get_value(RESULT_ENDPOINT) {
            Ok(body) => {
                let state = body.get("state").and_then(Value::as_str).unwrap_or("?");
                if state == "completed" {
                    emit_status(&body, json_out)?;
                    return exit_for(&body);
                }
                if Instant::now() > deadline {
                    bail!("timeout ({}s) — 最後の状態: {state}", args.timeout);
                }
            }
            // DomainReload 中は接続できない。タイムアウトまでは正常系として扱う。
            Err(_) if Instant::now() < deadline => continue,
            Err(_) => bail!(
                "timeout ({}s): 結果取得中に接続できませんでした",
                args.timeout
            ),
        }
    }
}

// サーバが 409 (既に実行中) を返したか。
fn is_already_running(e: &anyhow::Error) -> bool {
    e.downcast_ref::<crate::http::HttpError>()
        .is_some_and(|h| h.status == 409)
}

fn mode_str(m: TestMode) -> &'static str {
    match m {
        TestMode::Playmode => "playmode",
        TestMode::Editmode => "editmode",
        TestMode::Result => "result",
    }
}

fn emit_status(body: &Value, json_out: bool) -> Result<()> {
    if json_out {
        println!("{}", serde_json::to_string_pretty(body)?);
        return Ok(());
    }

    let state = body.get("state").and_then(Value::as_str).unwrap_or("?");
    let mode = body.get("mode").and_then(Value::as_str).unwrap_or("?");
    match state {
        "idle" => {
            println!("{DIM}(まだテストを実行していません){DIM:#}");
            return Ok(());
        }
        "running" => {
            println!("{YELLOW}running{YELLOW:#}  {mode}");
            return Ok(());
        }
        _ => {}
    }

    let result = body.get("result").and_then(Value::as_str).unwrap_or("?");
    let head = if result == "Passed" {
        format!("{GREEN}PASS{GREEN:#}")
    } else {
        format!("{RED}FAIL{RED:#}")
    };
    let dur = body
        .get("durationSeconds")
        .map(ToString::to_string)
        .unwrap_or_else(|| "?".into());
    println!("{head}  {result}  [{mode}]  ({dur}s)");

    let num = |k: &str| body.get(k).map(ToString::to_string).unwrap_or_default();
    let failed_n = body.get("failed").and_then(Value::as_u64).unwrap_or(0);
    println!("  passed       : {}", num("passed"));
    if failed_n > 0 {
        println!("  failed       : {RED}{}{RED:#}", num("failed"));
    } else {
        println!("  failed       : {}", num("failed"));
    }
    println!("  skipped      : {}", num("skipped"));
    println!("  inconclusive : {}", num("inconclusive"));

    let failures = body
        .get("failures")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !failures.is_empty() {
        println!();
        for f in &failures {
            let name = f.get("name").and_then(Value::as_str).unwrap_or("?");
            println!("  {RED}✗{RED:#} {name}");
            let msg = f.get("message").and_then(Value::as_str).unwrap_or("");
            for line in msg.trim().lines() {
                println!("      {DIM}{line}{DIM:#}");
            }
        }
    }
    // サーバは先頭 N 件しか返さないので、全部見えていないことを知らせる。
    if failed_n > 0 && (failures.len() as u64) < failed_n {
        println!("{DIM}  (詳細は先頭 {} 件のみ){DIM:#}", failures.len());
    }
    Ok(())
}

// 完了して Passed のときだけ成功扱い。running / idle は失敗ではない。
fn exit_for(body: &Value) -> Result<()> {
    let completed = body.get("state").and_then(Value::as_str) == Some("completed");
    let passed = body.get("result").and_then(Value::as_str) == Some("Passed");
    if completed && !passed {
        return Err(ExecFailure.into());
    }
    Ok(())
}
