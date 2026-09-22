//! TESTS.md「仕上げ: e2e と総合」— ヘルプ / 引数パース / 副作用なし / exit code の回帰

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::{cmd, free_port, health_body, project_config_path, temp_home, temp_project};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use predicates::prelude::*;

// ---- clap 引数 ----

#[test]
fn help_は_exit0_でサブコマンド一覧を出す() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("health"))
        .stdout(predicate::str::contains("exec"))
        .stdout(predicate::str::contains("run"));
}

#[test]
fn サブコマンドのhelpも_exit0() {
    cmd()
        .args(["exec", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("KEY=VALUE").or(predicate::str::contains("args")));
}

#[test]
fn version_は_exit0() {
    cmd().arg("--version").assert().success();
}

#[test]
fn サブコマンド未指定はエラー() {
    // SPEC §11: 引数エラーは 1 (clap 既定の 2 ではない)
    cmd().assert().code(1);
}

#[test]
fn 未知のサブコマンドはエラー() {
    cmd().arg("nonexistent").assert().code(1);
}

#[test]
fn mode_の値が不正ならエラー() {
    cmd().args(["--mode", "foo", "health"]).assert().code(1);
}

#[test]
fn グローバルフラグはサブコマンドの後ろにも置ける() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    // --json を後置しても前置と同じ結果になる
    cmd()
        .args(["health", "--base-url", &server.base_url(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""));
}

// ---- 副作用 ----

#[test]
fn 通常コマンドは_ProjectSettings_を書かない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });
    let tmp = temp_project();

    cmd()
        .current_dir(tmp.path())
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success();

    assert!(
        !project_config_path(tmp.path()).exists(),
        "health が設定ファイルを作っている"
    );
}

#[test]
fn base_url指定時はキャッシュを書かない() {
    // discovery を通らない経路ではキャッシュ更新も起きない
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });
    let home = temp_home();

    cmd()
        .env("HOME", home.path())
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success();

    assert!(
        !home.path().join(".liminal-palette/ports.json").exists(),
        "--base-url 指定なのにキャッシュが書かれている"
    );
}

#[test]
fn discovery_で当たったポートはキャッシュに残る() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "Cached", "/dev/Cached"));
    });
    let home = temp_home();

    cmd()
        .env("HOME", home.path())
        .args(["--port", &server.port().to_string(), "health"])
        .assert()
        .success();

    let raw = std::fs::read_to_string(home.path().join(".liminal-palette/ports.json"))
        .expect("キャッシュが書かれていない");
    assert!(raw.contains("/dev/Cached"), "{raw}");
    assert!(raw.contains(&server.port().to_string()), "{raw}");
}

#[test]
fn init_はフラグなしなら何も書かない() {
    let tmp = temp_project();
    let home = temp_home();

    cmd()
        .current_dir(tmp.path())
        .env("HOME", home.path())
        .arg("init")
        .assert()
        .success();

    assert!(!project_config_path(tmp.path()).exists());
}

// ---- exit code 一覧の回帰 (TESTS.md「仕上げ」) ----

#[test]
fn exit_code_health成功は0() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });
    cmd()
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .code(0);
}

#[test]
fn exit_code_接続失敗は1() {
    cmd()
        .args(["--port", &free_port().to_string(), "health"])
        .assert()
        .code(1);
}

#[test]
fn exit_code_exec成功は0_失敗は2() {
    let ok = MockServer::start();
    ok.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .body(r#"{"success":true,"value":"1","durationMs":1.0,"logs":[]}"#);
    });
    cmd()
        .args(["--base-url", &ok.base_url(), "exec", "Foo"])
        .assert()
        .code(0);

    let ng = MockServer::start();
    ng.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(200)
            .body(r#"{"success":false,"error":"boom","durationMs":1.0,"logs":[]}"#);
    });
    cmd()
        .args(["--base-url", &ng.base_url(), "exec", "Foo"])
        .assert()
        .code(2);
}

#[test]
fn exit_code_exec_HTTP500は1() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/execute");
        then.status(500).body("boom");
    });
    cmd()
        .args(["--base-url", &server.base_url(), "exec", "Foo"])
        .assert()
        .code(1);
}

#[test]
fn exit_code_doctorは常に0() {
    let tmp = temp_project();
    let home = temp_home();
    cmd()
        .current_dir(tmp.path())
        .env("HOME", home.path())
        .args(["--port", &free_port().to_string(), "doctor"])
        .assert()
        .code(0);
}

#[test]
fn exit_code_init_非Unityは1() {
    let tmp = tempfile::TempDir::new().unwrap();
    cmd().current_dir(tmp.path()).arg("init").assert().code(1);
}

#[test]
fn exit_code_set_port_範囲外は1() {
    let tmp = temp_project();
    cmd()
        .current_dir(tmp.path())
        .args(["project", "set-port", "70000"])
        .assert()
        .code(1);
}

// ---- 色の抑制 (SPEC §10) ----

/// 色を抑制していない素の Command (cmd() は NO_COLOR=1 を立ててしまうため)
fn raw_cmd() -> assert_cmd::Command {
    // cmd() から NO_COLOR だけ外した版 (色が出ないことを確かめるため)
    let mut c = cmd();
    c.env_remove("NO_COLOR");
    c
}

#[test]
fn NO_COLOR_ならカラーコードが出ない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    raw_cmd()
        .env("NO_COLOR", "1")
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{1b}[").not());
}

#[test]
fn 非TTYならカラーコードが出ない() {
    // assert_cmd は stdout をパイプで受けるので TTY ではない。
    // NO_COLOR を立てなくても色が出ないこと。
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    raw_cmd()
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{1b}[").not());
}

#[test]
fn json_出力にカラーコードが混ざらない() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    raw_cmd()
        .args(["--base-url", &server.base_url(), "--json", "health"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{1b}[").not());
}

#[test]
fn エラー出力も非TTYなら色無し() {
    raw_cmd()
        .args(["--port", &free_port().to_string(), "health"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("\u{1b}[").not());
}

// ---- トークン未設定 (SPEC §11) ----

#[test]
fn 認証必須コマンドはトークン未設定なら実行前に落ちる() {
    // サーバに投げて 401 を見るのではなく、何を直せばよいかを示して止まる。
    // HOME を差し替えないと home_dir() が passwd から実ホームを拾い、
    // 実環境の ~/.liminal-palette/token を読んでしまう。
    let home = temp_home();
    common::cmd_without_token()
        .env("HOME", home.path())
        .args(["--base-url", "http://127.0.0.1:1", "commands"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("トークンが見つかりません"))
        .stderr(predicate::str::contains("LP_TOKEN"));
}

#[test]
fn health_はトークン未設定でも動く() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    let home = temp_home();
    common::cmd_without_token()
        .env("NO_COLOR", "1")
        .env("HOME", home.path())
        .args(["--base-url", &server.base_url(), "health"])
        .assert()
        .success();
}

#[test]
fn health_はトークンがあっても認証ヘッダを送らない() {
    // SPEC §4.1: /health は認証不要。余計な Authorization を付けない
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/api/v1/health").matches(|req| {
            !req.headers.as_ref().is_some_and(|hs| {
                hs.iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            })
        });
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    cmd()
        .args(["--base-url", &server.base_url(), "--token", "abc", "health"])
        .assert()
        .success();

    m.assert();
}

// ---- 副作用の細部 ----

#[test]
fn init_はフラグなしなら_ProjectSettings_の更新時刻も変えない() {
    let tmp = temp_project();
    let home = temp_home();
    let ps = tmp.path().join("ProjectSettings");
    let before = std::fs::metadata(&ps).unwrap().modified().unwrap();

    cmd()
        .current_dir(tmp.path())
        .env("HOME", home.path())
        .arg("init")
        .assert()
        .success();

    let after = std::fs::metadata(&ps).unwrap().modified().unwrap();
    assert_eq!(before, after, "init が ProjectSettings を触っている");
}

#[test]
fn キャッシュが書けない状況でも落ちない() {
    // 書き込み権限の無い HOME を与えても、キャッシュ保存の失敗は握り潰す
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).body(health_body("editor", "P", "/p"));
    });

    let home = tempfile::TempDir::new().unwrap();
    let lp = home.path().join(".liminal-palette");
    std::fs::create_dir_all(&lp).unwrap();
    let mut perm = std::fs::metadata(&lp).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o555);
    }
    std::fs::set_permissions(&lp, perm).unwrap();

    cmd()
        .env("HOME", home.path())
        .args(["--port", &server.port().to_string(), "health"])
        .assert()
        .success();
}

#[test]
fn キャッシュ一致なら余計なポートを叩かない() {
    // target 一致のキャッシュがあるときは、そのポートだけで確定する (SPEC §5-4)
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200)
            .body(health_body("editor", "Fast", "/dev/Fast"));
    });

    let home = temp_home();
    common::write_port_cache(home.path(), "/dev/Fast", "Fast", server.port());

    cmd()
        .env("HOME", home.path())
        .args(["--project", "Fast", "health"])
        .assert()
        .success();

    // 候補ポートの総当たりに入らず 1 回で済む
    assert_eq!(m.hits(), 1, "キャッシュ早出しが効いていない");
}
