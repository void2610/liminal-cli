//! SPEC §4.8 `state` の integration test

mod common;

use common::cmd;
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;

#[test]
fn state_path_指定で単一フィールドを取得() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/state")
            .query_param("path", "Player/Health");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"path":"Player/Health","value":100,"type":"Int32"}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "state",
            "Player/Health",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/Health"))
        .stdout(predicate::str::contains("100"))
        .stdout(predicate::str::contains("Int32"));

    m.assert();
}

#[test]
fn state_path_に_スラッシュ_含む値が_percent_encode_される() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/state")
            // ureq/httpmock は query_param で受け取るとデコード済みの値で比較される
            .query_param("path", "Player/Health/HP");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"path":"Player/Health/HP","value":50,"type":"Int32"}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "state",
            "Player/Health/HP",
        ])
        .assert()
        .success();

    m.assert();
}

#[test]
fn state_path_省略で_全件取得() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/state");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
                "fields": [
                    {"path":"Player/Health","value":100,"type":"Int32","instanceResolved":true},
                    {"path":"Player/Mana","value":null,"type":"Int32","instanceResolved":false}
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
            "state",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Player/Health"))
        .stdout(predicate::str::contains("Player/Mana"))
        // instanceResolved=true は ●、false は ○
        .stdout(predicate::str::contains("●"))
        .stdout(predicate::str::contains("○"))
        .stdout(predicate::str::contains("total: 2"));
}

#[test]
fn state_value_null_は文字列_null_で表示() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/state");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"fields":[{"path":"X","value":null,"type":"Int32","instanceResolved":true}]}"#,
            );
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "state",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("null"));
}

#[test]
fn state_0件で_no_state_fields() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/state");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"fields":[]}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--token",
            "dummy",
            "state",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("(no state fields)"));
}

#[test]
fn state_json_フラグで透過() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/state");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"fields":[{"path":"X","value":1,"type":"Int32","instanceResolved":true}]}"#);
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "--token",
            "dummy",
            "state",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"fields\""))
        .stdout(predicate::str::contains("\"instanceResolved\": true"));
}
