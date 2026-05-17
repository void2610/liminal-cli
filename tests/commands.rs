//! SPEC §4.5 `commands` の integration test

mod common;

use common::cmd;
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;

/// commands エンドポイントの典型レスポンス
const COMMANDS_BODY: &str = r#"{
  "commands": [
    {
      "path": "Player/Health/Heal",
      "name": "Heal",
      "category": "Player",
      "description": "HP を回復する",
      "isAsync": false,
      "returnType": "void",
      "aliases": [],
      "parameters": [
        {"name":"amount","type":"Int32","position":0,"hasDefault":false}
      ]
    },
    {
      "path": "Player/Health/FullHeal",
      "name": "FullHeal",
      "category": "Player",
      "description": "HP を最大まで回復する",
      "isAsync": false,
      "returnType": "void",
      "aliases": [],
      "parameters": []
    },
    {
      "path": "Editor/Console/Clear",
      "name": "Clear",
      "category": "Editor",
      "description": "Console をクリア",
      "isAsync": false,
      "returnType": "void",
      "aliases": [],
      "parameters": []
    }
  ]
}"#;

#[test]
fn commands_全件取得して_total_を表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(200)
            .header("content-type", "application/json")
            .body(COMMANDS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "commands",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/Health/Heal"))
        .stdout(predicate::str::contains("Player/Health/FullHeal"))
        .stdout(predicate::str::contains("Editor/Console/Clear"))
        .stdout(predicate::str::contains("total: 3"));
}

#[test]
fn commands_filter_prefix_で絞り込まれる() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(200)
            .header("content-type", "application/json")
            .body(COMMANDS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "commands",
            "--filter",
            "Player/",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/Health/Heal"))
        .stdout(predicate::str::contains("Player/Health/FullHeal"))
        // Editor/ は filter で外れる
        .stdout(predicate::str::contains("Editor/Console/Clear").not())
        // total はフィルタ後の件数
        .stdout(predicate::str::contains("total: 2"));
}

#[test]
fn commands_filter_後_0件で_no_commands_を表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(200)
            .header("content-type", "application/json")
            .body(COMMANDS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "commands",
            "--filter",
            "Nonexistent/",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("(no commands)"));
}

#[test]
fn commands_パラメータ表示は_name_type_形式() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(200)
            .header("content-type", "application/json")
            .body(COMMANDS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "commands",
        ])
        .assert()
        .success()
        // Heal は (amount:Int32) を持つ
        .stdout(predicate::str::contains("(amount:Int32)"));
}

#[test]
fn commands_json_フラグでフィルタ後の_json_を出す() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/commands");
        then.status(200)
            .header("content-type", "application/json")
            .body(COMMANDS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "--token",
            "dummy",
            "commands",
            "--filter",
            "Player/",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/Health/Heal"))
        .stdout(predicate::str::contains("Editor/Console/Clear").not())
        .stdout(predicate::str::contains("\"commands\""));
}

#[test]
fn commands_は_authorization_bearer_を送る() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/commands")
            .header("Authorization", "Bearer testtoken");
        then.status(200)
            .header("content-type", "application/json")
            .body(COMMANDS_BODY);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "testtoken",
            "commands",
        ])
        .assert()
        .success();

    m.assert();
}
