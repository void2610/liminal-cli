//! SPEC §5 discovery の integration test

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::cmd;
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;
use std::net::TcpListener;
use tempfile::TempDir;

fn health_body(mode: &str, name: &str, path: &str) -> String {
    format!(
        r#"{{"status":"ok","version":"0.2.0","mode":"{mode}","projectName":"{name}",
             "projectPath":"{path}","commandCount":1}}"#
    )
}

/// 誰も listen していないポートを 1 つ得る (bind して即 drop する)。
fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

/// Unity プロジェクトに見える一時ディレクトリ (preferred port 付き)。
fn project_with_port(port: u16) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let ps = tmp.path().join("ProjectSettings");
    std::fs::create_dir_all(&ps).unwrap();
    std::fs::write(
        ps.join("ProjectVersion.txt"),
        "m_EditorVersion: 6000.0.0f1\n",
    )
    .unwrap();
    std::fs::write(
        ps.join("LiminalPalette.json"),
        format!(r#"{{"port":{port}}}"#),
    )
    .unwrap();
    tmp
}

#[test]
fn discovery_port指定でそのポートだけを使う() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    cmd()
        .args(["--port", &server.port().to_string(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("P"));
}

#[test]
fn discovery_生存ゼロなら試したポートを添えて_exit1() {
    let port = free_port();
    cmd()
        .args(["--port", &port.to_string(), "health"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("見つかりません"))
        .stderr(predicate::str::contains(port.to_string()));
}

#[test]
fn discovery_preferred_port_を候補に含める() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "Pref", "/pref"));
    });
    let tmp = project_with_port(server.port());

    // SPEC §5-5 は候補を全部 probe してから選ぶ (生存が複数なら曖昧エラーにするため)。
    // テストは他テストの mock サーバも同時に生きているので、--project で対象を確定させる。
    cmd()
        .current_dir(tmp.path())
        .args(["--project", "Pref", "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pref"));

    assert!(m.hits() >= 1, "preferred port が probe されていない");
}

#[test]
fn discovery_project不一致は致命エラー() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "Actual", "/actual"));
    });

    cmd()
        .args([
            "--port",
            &server.port().to_string(),
            "--project",
            "Nope",
            "health",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("指定のプロジェクト 'Nope'"));
}

#[test]
fn discovery_mode不一致は致命エラー() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    cmd()
        .args([
            "--port",
            &server.port().to_string(),
            "--mode",
            "runtime",
            "health",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("mode=runtime"));
}

#[test]
fn discovery_projectName一致なら採用される() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "MyGame", "/dev/MyGame"));
    });

    cmd()
        .args([
            "--port",
            &server.port().to_string(),
            "--project",
            "MyGame",
            "health",
        ])
        .assert()
        .success();
}

#[test]
fn discovery_非JSON応答のポートは生存扱いしない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body("not json");
    });

    cmd()
        .args(["--port", &server.port().to_string(), "health"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("見つかりません"));
}

#[test]
fn discovery_JSON配列応答も生存扱いしない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body("[1,2,3]");
    });

    cmd()
        .args(["--port", &server.port().to_string(), "health"])
        .assert()
        .code(1);
}

#[test]
fn discovery_base_url指定は_discovery_をバイパスする() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "Direct", "/d"));
    });

    // preferred port は全く別 (到達不能) でも --base-url が勝つ
    let tmp = project_with_port(free_port());

    cmd()
        .current_dir(tmp.path())
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Direct"));
}
