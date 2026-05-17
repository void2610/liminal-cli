//! SPEC §4.6 `exec` の integration test (POST + exit code 2)

mod common;

use common::cmd;
use httpmock::Method::POST;
use httpmock::MockServer;
use predicates::prelude::*;

#[test]
fn exec_success_時は_exit_0_と_success_表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": true,
                "value": 42,
                "durationMs": 1.07,
                "logs": []
            }"#,
            );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "exec",
            "Player/Health/Heal",
            "amount=10",
        ])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("success"))
        .stdout(predicate::str::contains("1.07 ms"));
}

#[test]
fn exec_failure_時は_exit_2() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": false,
                "error": "command not found",
                "durationMs": 0.5,
                "logs": []
            }"#,
            );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "exec",
            "NonExistent",
        ])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("failed"))
        .stdout(predicate::str::contains("command not found"));
}

#[test]
fn exec_failure_でも_json_フラグなら_json_を出して_exit_2() {
    // SPEC §4.6: `--json` でも success=false は exit 2
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": false,
                "error": "boom",
                "durationMs": 0.0,
                "logs": []
            }"#,
            );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "--token",
            "dummy",
            "exec",
            "NonExistent",
        ])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("\"success\": false"))
        .stdout(predicate::str::contains("\"error\": \"boom\""));
}

#[test]
fn exec_は_path_と_args_を_post_body_に乗せる() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/execute")
            .header("Authorization", "Bearer testtoken")
            .json_body(serde_json::json!({
                "path": "Player/Health/Set",
                "args": {"value": "100", "kind": "absolute"}
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success":true,"durationMs":1.0,"logs":[]}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "testtoken",
            "exec",
            "Player/Health/Set",
            "value=100",
            "kind=absolute",
        ])
        .assert()
        .success();

    m.assert();
}

#[test]
fn exec_値側に等号があっても最初の等号で分割される() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/execute")
            .json_body(serde_json::json!({
                "path": "Foo",
                "args": {"expr": "a=b"}
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success":true,"durationMs":0.1,"logs":[]}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "exec",
            "Foo",
            "expr=a=b",
        ])
        .assert()
        .success();

    m.assert();
}

#[test]
fn exec_引数_等号無しは_clap_エラーで_exit_2() {
    // clap の引数パースエラーは exit 2 (clap 既定) になる
    cmd()
        .args([
            "--base-url",
            "http://127.0.0.1:1",
            "--token",
            "dummy",
            "exec",
            "Foo",
            "novalue",
        ])
        .assert()
        .failure();
}

#[test]
fn exec_logs_の色_type_ごとに表示される() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "success": true,
                "durationMs": 1.0,
                "logs": [
                    {"type":"Log","message":"hello","timestamp":"2024-01-01T00:00:00Z"},
                    {"type":"Warning","message":"careful","timestamp":"2024-01-01T00:00:01Z"},
                    {"type":"Error","message":"oops","timestamp":"2024-01-01T00:00:02Z"}
                ]
            }"#,
            );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "exec",
            "Foo",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("logs (3):"))
        .stdout(predicate::str::contains("Log: hello"))
        .stdout(predicate::str::contains("Warning: careful"))
        .stdout(predicate::str::contains("Error: oops"));
}

#[test]
fn exec_logs_0件は_logs_ブロックを出さない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"success":true,"durationMs":1.0,"logs":[]}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "exec",
            "Foo",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("logs (").not());
}
