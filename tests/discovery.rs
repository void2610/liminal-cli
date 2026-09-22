//! SPEC §5 discovery の integration test

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::{cmd, free_port, health_body, temp_home, temp_project_with_port, write_port_cache};
use httpmock::Method::GET;
use httpmock::MockServer;
use predicates::prelude::*;
use tempfile::TempDir;

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
    let tmp = temp_project_with_port(server.port());

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
    let tmp = temp_project_with_port(free_port());

    cmd()
        .current_dir(tmp.path())
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Direct"));
}

#[test]
fn discovery_port_0_は拒否する() {
    // u16 では 0 を弾けないので、範囲検証が別途効いている必要がある
    cmd()
        .args(["--port", "0", "health"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("1..=65535"));
}

#[test]
fn discovery_port_明示時はキャッシュ早出しをしない() {
    // キャッシュに載っている別ポートではなく、指定したポートだけを使うこと
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "MyGame", "/dev/MyGame"));
    });

    // キャッシュには死んでいるポートを載せておく
    let home = temp_home();
    write_port_cache(home.path(), "/dev/MyGame", "MyGame", free_port());

    // キャッシュ上の (死んでいる) ポートに引っぱられず、--port が勝つ
    cmd()
        .env("HOME", home.path())
        .args([
            "--port",
            &server.port().to_string(),
            "--project",
            "MyGame",
            "health",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("MyGame"));
}

#[test]
fn discovery_cwdのプロジェクトが既定のターゲットになる() {
    // cwd が Unity プロジェクトなら、別プロジェクトのサーバは選ばれない (SPEC §2)
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "Other", "/dev/Other"));
    });
    let tmp = temp_project_with_port(server.port());

    cmd()
        .current_dir(tmp.path())
        .arg("health")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("指定のプロジェクト"));
}

#[test]
fn discovery_project指定時はそのディレクトリの_preferred_を使う() {
    // cwd の外にあるプロジェクトを --project で指定しても、そのプロジェクトの
    // preferred port が候補に入ること (既定ポートやキャッシュに無くても届く)
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "Far", "/dev/Far"));
    });
    let target = temp_project_with_port(server.port());
    let elsewhere = TempDir::new().unwrap();

    cmd()
        .current_dir(elsewhere.path())
        .args(["--project", target.path().to_str().unwrap(), "health"])
        .assert()
        // target のパスは実在ディレクトリなので canonicalize され、/dev/Far とは一致しない。
        // ここで確認したいのは「preferred port が probe されたか」。
        .code(1);

    assert!(
        m.hits() >= 1,
        "target の preferred port が probe されていない"
    );
}
