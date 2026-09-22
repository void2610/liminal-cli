//! SPEC §6 の HTTP 契約 (Accept ヘッダ / エラー表示) と `--json` の丸出しを確認する

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::cmd;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use predicates::prelude::*;

#[test]
fn エラー時はサーバのメッセージを添えて表示する() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(401).body(r#"{"error":"token が一致しません"}"#);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "commands"])
        .assert()
        .code(1)
        // SPEC §6: HTTP {status}: {body.error}
        .stderr(predicate::str::contains("HTTP 401"))
        .stderr(predicate::str::contains("token が一致しません"));
}

#[test]
fn エラーbodyが_JSON_でなければそのまま載せる() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(500).body("internal server error");
    });

    cmd()
        .args(["--base-url", &server.base_url(), "commands"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("HTTP 500"))
        .stderr(predicate::str::contains("internal server error"));
}

#[test]
fn エラーbodyが空でも壊れない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(503).body("");
    });

    cmd()
        .args(["--base-url", &server.base_url(), "commands"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("HTTP 503"));
}

#[test]
fn GET_は_Accept_ヘッダを送る() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/commands")
            .header("Accept", "application/json");
        then.status(200).body(r#"{"commands":[]}"#);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "commands"])
        .assert()
        .success();

    m.assert();
}

#[test]
fn POST_も_Accept_ヘッダを送る() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/execute")
            .header("Accept", "application/json");
        then.status(200)
            .body(r#"{"success":true,"value":null,"durationMs":1.0,"logs":[]}"#);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "exec", "Foo/Bar"])
        .assert()
        .success();

    m.assert();
}

#[test]
fn json_はサーバが増やしたフィールドを落とさない() {
    // 型に定義していないフィールド (origin) が素通りすること。
    // 構造体を再シリアライズすると消えてしまい、jq で絞れなくなる。
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200).body(
            r#"{"invocations":[{"path":"Foo/Bar","timestamp":"2026-09-22T00:00:00Z","origin":"ipc",
                 "args":{},"result":{"success":true,"value":null,"durationMs":1.0,"logs":[]}}],
                 "total":1,"limit":20,"serverExtra":"kept"}"#,
        );
    });

    cmd()
        .args(["--base-url", &server.base_url(), "--json", "logs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"origin\": \"ipc\""))
        .stdout(predicate::str::contains("\"serverExtra\": \"kept\""));
}

#[test]
fn json_の_health_も丸出しする() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(
            r#"{"status":"ok","version":"0.2.0","mode":"editor","projectName":"P",
                 "projectPath":"/p","commandCount":1,"futureField":42}"#,
        );
    });

    cmd()
        .args(["--base-url", &server.base_url(), "--json", "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"futureField\": 42"));
}

#[test]
fn json_の_filter_は絞り込み後を出す() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(200).body(
            r#"{"commands":[
                 {"path":"Player/A","name":"A","category":"c","isAsync":false,"returnType":"void","extra":1},
                 {"path":"Enemy/B","name":"B","category":"c","isAsync":false,"returnType":"void"}
               ]}"#,
        );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "commands",
            "--filter",
            "Player/",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/A"))
        .stdout(predicate::str::contains("Enemy/B").not())
        // 絞り込んでも未知フィールドは保たれる
        .stdout(predicate::str::contains("\"extra\": 1"));
}

#[test]
fn json_の_exec_は失敗でも丸出しして_exit2() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200).body(
            r#"{"success":false,"value":null,"error":"boom","durationMs":1.0,"logs":[],"hint":"x"}"#,
        );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "exec",
            "Foo/Bar",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("\"hint\": \"x\""));
}

#[test]
fn run_の_409_と_429_も_exit1_で理由を出す() {
    for (status, msg) in [
        (409u16, "別のシナリオが実行中です"),
        (429, "Rate limit exceeded"),
    ] {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/scenarios/run");
            then.status(status).body(format!(r#"{{"error":"{msg}"}}"#));
        });

        cmd()
            .args(["--base-url", &server.base_url(), "run", "Foo/Bar"])
            .assert()
            .code(1)
            .stderr(predicate::str::contains(format!("HTTP {status}")))
            .stderr(predicate::str::contains(msg));
    }
}

#[test]
fn probe_は応答が遅いポートを生存扱いしない() {
    // discovery の probe は 0.4s で諦める (SPEC §2)。
    // 立っているが返さないポートに引きずられないことの回帰テスト。
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .delay(std::time::Duration::from_millis(1500))
            .body(r#"{"status":"ok"}"#);
    });

    cmd()
        .args(["--port", &server.port().to_string(), "health"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("見つかりません"));
}
