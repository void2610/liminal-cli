//! `liminal test` (Unity Test Runner 連携) の integration test

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::cmd;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use predicates::prelude::*;

const COMPLETED_PASS: &str = r#"{"state":"completed","mode":"editmode","result":"Passed",
  "passed":744,"failed":0,"skipped":0,"inconclusive":0,"durationSeconds":31.2,"failures":[]}"#;

const COMPLETED_FAIL: &str = r#"{"state":"completed","mode":"editmode","result":"Failed",
  "passed":740,"failed":2,"skipped":0,"inconclusive":0,"durationSeconds":31.2,
  "failures":[{"name":"Foo.BarTest","message":"expected 1 but was 2"}]}"#;

#[test]
fn test_editmode_は成功で_exit0() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/tests/run");
        then.status(200)
            .body(r#"{"mode":"editmode","filter":"all"}"#);
    });
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(COMPLETED_PASS);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "test",
            "editmode",
            "--interval",
            "0.05",
        ])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::str::contains("passed       : 744"));
}

#[test]
fn test_失敗は_exit2_で失敗テスト名を出す() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/tests/run");
        then.status(200)
            .body(r#"{"mode":"editmode","filter":"all"}"#);
    });
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(COMPLETED_FAIL);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "test",
            "editmode",
            "--interval",
            "0.05",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("Foo.BarTest"))
        .stdout(predicate::str::contains("expected 1 but was 2"));
}

#[test]
fn test_filter_を_body_に乗せる() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/tests/run")
            .json_body_partial(r#"{"mode":"playmode","filter":"Auction.*"}"#);
        then.status(200)
            .body(r#"{"mode":"playmode","filter":"Auction.*"}"#);
    });
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(COMPLETED_PASS);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "test",
            "playmode",
            "--filter",
            "Auction.*",
            "--interval",
            "0.05",
        ])
        .assert()
        .code(0);

    m.assert();
}

#[test]
fn test_result_は実行を開始しない() {
    let server = MockServer::start();
    let run = server.mock(|when, then| {
        when.method(POST).path("/api/v1/tests/run");
        then.status(200).body("{}");
    });
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(COMPLETED_PASS);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "test", "result"])
        .assert()
        .code(0);

    assert_eq!(run.hits(), 0, "result なのに実行を開始している");
}

#[test]
fn test_result_の_idle_は_exit0() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(r#"{"state":"idle"}"#);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "test", "result"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("まだテストを実行していません"));
}

#[test]
fn test_409_は進行中のランに相乗りする() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/tests/run");
        then.status(409).body(r#"{"error":"既にテスト実行中です"}"#);
    });
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(COMPLETED_PASS);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "test",
            "editmode",
            "--interval",
            "0.05",
        ])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("既にテスト実行中"))
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn test_409_と_no_wait_なら_exit2() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/tests/run");
        then.status(409).body(r#"{"error":"既にテスト実行中です"}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "test",
            "editmode",
            "--no-wait",
        ])
        .assert()
        .code(2);
}

#[test]
fn test_no_wait_は完了を待たない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/tests/run");
        then.status(200)
            .body(r#"{"mode":"editmode","filter":"all"}"#);
    });
    let poll = server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200).body(COMPLETED_PASS);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "test",
            "editmode",
            "--no-wait",
        ])
        .assert()
        .code(0);

    assert_eq!(poll.hits(), 0, "--no-wait なのに polling している");
}

#[test]
fn test_json_はレスポンスを丸出しする() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/tests/result");
        then.status(200)
            .body(r#"{"state":"completed","result":"Passed","futureField":1}"#);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "--json", "test", "result"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("\"futureField\": 1"));
}

#[test]
fn test_未対応のモードはエラー() {
    cmd()
        .args(["--base-url", "http://127.0.0.1:1", "test", "nope"])
        .assert()
        .code(1);
}
