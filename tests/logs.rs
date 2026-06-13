//! SPEC §4.7 `logs` の integration test

mod common;

use common::cmd;
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;

const LOGS_BODY: &str = r#"{
  "invocations": [
    {
      "path": "Player/Health/Heal",
      "timestamp": "2024-01-01T00:00:00Z",
      "args": {"amount": "10"},
      "result": {"success": true, "value": 100, "durationMs": 1.07, "logs": []}
    },
    {
      "path": "NonExistent/Cmd",
      "timestamp": "2024-01-01T00:00:01Z",
      "args": {},
      "result": {"success": false, "error": "not found", "durationMs": 0.3, "logs": []}
    }
  ],
  "total": 2,
  "limit": 20
}"#;

#[test]
fn logs_一覧と_shown_total_行() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200)
            .header("content-type", "application/json")
            .body(LOGS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "logs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/Health/Heal"))
        .stdout(predicate::str::contains("NonExistent/Cmd"))
        .stdout(predicate::str::contains("shown 2 / total 2"));
}

#[test]
fn logs_success_と_failure_でマーカーが分かれる() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200)
            .header("content-type", "application/json")
            .body(LOGS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "logs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓"))
        .stdout(predicate::str::contains("✗"));
}

#[test]
fn logs_args_あり時に_args_行を表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200)
            .header("content-type", "application/json")
            .body(LOGS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "logs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("amount=10"));
}

#[test]
fn logs_失敗時は_error_行を表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200)
            .header("content-type", "application/json")
            .body(LOGS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "logs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("error: not found"));
}

#[test]
fn logs_limit_指定時は_クエリ付きで叩く() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/logs")
            .query_param("limit", "10");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"invocations":[],"total":0,"limit":10}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "logs",
            "--limit",
            "10",
        ])
        .assert()
        .success();

    m.assert();
}

#[test]
fn logs_0件で_no_invocations() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"invocations":[],"total":0,"limit":20}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "logs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("(no invocations)"));
}

#[test]
fn logs_json_フラグで透過() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/logs");
        then.status(200)
            .header("content-type", "application/json")
            .body(LOGS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "--token",
            "dummy",
            "logs",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"invocations\""))
        .stdout(predicate::str::contains("\"total\": 2"));
}
