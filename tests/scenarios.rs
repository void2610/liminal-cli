//! SPEC §4.9 `scenarios` の integration test

mod common;

use common::cmd;
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;

const SCENARIOS_BODY: &str = r#"{
  "scenarios": [
    {"path":"Combat/EnemyTakesDamage","description":"敵が被弾する","stepCount":5},
    {"path":"Combat/PlayerHeals","description":"プレイヤーが回復","stepCount":3},
    {"path":"Repro/Bug123","description":"既知バグ再現","stepCount":-1}
  ]
}"#;

#[test]
fn scenarios_一覧表示と_total() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200)
            .header("content-type", "application/json")
            .body(SCENARIOS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "scenarios",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Combat/EnemyTakesDamage"))
        .stdout(predicate::str::contains("Combat/PlayerHeals"))
        .stdout(predicate::str::contains("Repro/Bug123"))
        .stdout(predicate::str::contains("total: 3"));
}

#[test]
fn scenarios_step_count_マイナス1は_question_mark_表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200)
            .header("content-type", "application/json")
            .body(SCENARIOS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "scenarios",
        ])
        .assert()
        .success()
        // stepCount: 5 / 3 / -1 → "[5 steps]" / "[3 steps]" / "[? steps]"
        .stdout(predicate::str::contains("[? steps]"))
        .stdout(predicate::str::contains("[5 steps]"));
}

#[test]
fn scenarios_filter_prefix_で絞り込み() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200)
            .header("content-type", "application/json")
            .body(SCENARIOS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "scenarios",
            "--filter",
            "Combat/",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Combat/EnemyTakesDamage"))
        .stdout(predicate::str::contains("Repro/Bug123").not())
        .stdout(predicate::str::contains("total: 2"));
}

#[test]
fn scenarios_0件で_no_scenarios() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"scenarios":[]}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "scenarios",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("(no scenarios)"));
}

#[test]
fn scenarios_json_フラグで透過() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200)
            .header("content-type", "application/json")
            .body(SCENARIOS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "--token",
            "dummy",
            "scenarios",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"scenarios\""))
        .stdout(predicate::str::contains("\"stepCount\": -1"));
}
