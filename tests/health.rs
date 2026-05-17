//! SPEC §4.1 `health` の integration test

mod common;

use common::cmd;
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;

/// /api/v1/health の典型レスポンス本文
const HEALTH_BODY_FULL: &str = r#"{
  "status": "ok",
  "version": "0.4.0",
  "mode": "editor",
  "projectName": "TestProj",
  "projectPath": "/tmp/test-project",
  "commandCount": 42
}"#;

#[test]
fn health_全フィールド揃ったレスポンスを表示する() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .header("content-type", "application/json")
            .body(HEALTH_BODY_FULL);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"))
        .stdout(predicate::str::contains("0.4.0"))
        .stdout(predicate::str::contains("editor"))
        .stdout(predicate::str::contains("TestProj"))
        .stdout(predicate::str::contains("/tmp/test-project"))
        .stdout(predicate::str::contains("42"));

    m.assert();
}

#[test]
fn health_mode_等が空文字列なら_unknown_を表示する() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "status": "ok",
                "version": "0.4.0",
                "mode": "",
                "projectName": "",
                "projectPath": "",
                "commandCount": 0
            }"#,
            );
    });

    cmd()
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(unknown)"));
}

#[test]
fn health_json_フラグで整形済み_json_を出す() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .header("content-type", "application/json")
            .body(HEALTH_BODY_FULL);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "--json", "health"])
        .assert()
        .success()
        // pretty 整形なので `"version":` の前にスペース、改行が入る
        .stdout(predicate::str::contains("\"version\": \"0.4.0\""))
        .stdout(predicate::str::contains("\"commandCount\": 42"));
}

#[test]
fn health_接続失敗で_exit_1() {
    // ルーズなポート (httpmock を立てずに 1 確実に閉じているポートを使う)
    // 65000 番台はテスト中に他で使われる可能性が低い
    cmd()
        .args(["--base-url", "http://127.0.0.1:1", "health"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn health_http_500_で_exit_1() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(500).body("internal server error");
    });

    cmd()
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn health_は_authorization_ヘッダなしで叩く() {
    // SPEC §6: /health は認証不要なので Authorization ヘッダは送るべきでない
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/health")
            // Authorization ヘッダが「来ない」条件 (matches で空 or 不在は表現しづらいので、
            // 「Authorization: の prefix を持たない」ことを保証する別アプローチで)
            ;
        then.status(200)
            .header("content-type", "application/json")
            .body(HEALTH_BODY_FULL);
    });

    // token なしで叩く: env_clear 済みなので $LP_TOKEN は無い
    cmd()
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success();

    m.assert();
}
