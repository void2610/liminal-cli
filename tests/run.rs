//! SPEC §4.10 `run` / §7 JUnit XML の integration test

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::cmd;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use predicates::prelude::*;

fn pass_body(path: &str) -> String {
    format!(
        r#"{{"success":true,"durationMs":12.5,"failedAtStep":-1,"path":"{path}","alreadyRunning":false,
             "steps":[{{"kind":"Command","success":true,"durationMs":1.2,"commandPath":"Enemy/Spawn"}}]}}"#
    )
}

const FAIL_BODY: &str = r#"{"success":false,"durationMs":35.0,"failedAtStep":1,"path":"Combat/Heals","alreadyRunning":false,
  "steps":[
    {"kind":"Command","success":true,"durationMs":1.2,"commandPath":"Enemy/Spawn"},
    {"kind":"AssertEquals","success":false,"durationMs":0.1,"error":"expected '70' but got '65'","actualValue":65,"expected":"70"}
  ]}"#;

const SCENARIOS_BODY: &str = r#"{"scenarios":[
  {"path":"Battle/Alpha","description":"a","stepCount":2},
  {"path":"Battle/Repro/Beta","description":"b","stepCount":3},
  {"path":"Other/Gamma","description":"c","stepCount":1}
]}"#;

#[test]
fn run_named_成功は_PASS_を表示して_exit0() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(pass_body("Combat/Dies"));
    });

    cmd()
        .args(["--base-url", &server.base_url(), "run", "Combat/Dies"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::str::contains("Combat/Dies"))
        .stdout(predicate::str::contains("Enemy/Spawn"));
}

#[test]
fn run_named_失敗は_FAIL_と_failedAtStep_を表示して_exit2() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(FAIL_BODY);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "run", "Combat/Heals"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("failedAtStep: 1"))
        .stdout(predicate::str::contains("expected '70' but got '65'"));
}

#[test]
fn run_glob_はスラッシュを跨いで一致し集計を出す() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200).body(SCENARIOS_BODY);
    });
    let run_mock = server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(pass_body("x"));
    });

    cmd()
        .args(["--base-url", &server.base_url(), "run", "Battle/*"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Battle/Alpha"))
        // `*` が `/` を跨ぐ Python fnmatch 仕様
        .stdout(predicate::str::contains("Battle/Repro/Beta"))
        .stdout(predicate::str::contains("Other/Gamma").not())
        .stdout(predicate::str::contains("2 scenarios, 2 passed, 0 failed"));

    assert_eq!(run_mock.hits(), 2);
}

#[test]
fn run_glob_が0件ならエラーで_exit1() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200).body(SCENARIOS_BODY);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "run", "Nope/*"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("一致するシナリオがありません"));
}

#[test]
fn run_glob_で1件でも失敗すれば_exit2() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200).body(SCENARIOS_BODY);
    });
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(FAIL_BODY);
    });

    cmd()
        .args(["--base-url", &server.base_url(), "run", "Battle/*"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("0 passed, 2 failed"));
}

#[test]
fn run_steps_はファイルの配列を中継する() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/scenarios/run")
            .json_body_partial(r#"{"steps":[{"type":"command","path":"Foo/Bar"}]}"#);
        then.status(200).body(pass_body("adhoc"));
    });

    let tmp = tempfile::TempDir::new().unwrap();
    let f = tmp.path().join("steps.json");
    std::fs::write(&f, r#"[{"type":"command","path":"Foo/Bar"}]"#).unwrap();

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "run",
            "--steps",
            f.to_str().unwrap(),
        ])
        .assert()
        .success();

    m.assert();
}

#[test]
fn run_steps_は_steps_キー形式も受け付ける() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(pass_body("adhoc"));
    });

    let tmp = tempfile::TempDir::new().unwrap();
    let f = tmp.path().join("steps.json");
    std::fs::write(&f, r#"{"steps":[{"type":"wait_frames","frames":1}]}"#).unwrap();

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "run",
            "--steps",
            f.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn run_steps_は標準入力からも読める() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(pass_body("adhoc"));
    });

    cmd()
        .args(["--base-url", &server.base_url(), "run", "--steps", "-"])
        .write_stdin(r#"[{"type":"wait_seconds","seconds":0.1}]"#)
        .assert()
        .success();
}

#[test]
fn run_path_と_steps_の同時指定はエラー() {
    cmd()
        .args([
            "--base-url",
            "http://127.0.0.1:1",
            "run",
            "Foo/Bar",
            "--steps",
            "-",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("同時に指定できません"));
}

#[test]
fn run_引数なしはエラー() {
    cmd()
        .args(["--base-url", "http://127.0.0.1:1", "run"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("どちらかを指定してください"));
}

#[test]
fn run_report_で_JUnit_XML_を書き出す() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(FAIL_BODY);
    });

    let tmp = tempfile::TempDir::new().unwrap();
    // 親ディレクトリが無くても自動作成される
    let report = tmp.path().join("nested/report.xml");

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "run",
            "Combat/Heals",
            "--report",
            report.to_str().unwrap(),
        ])
        .assert()
        .code(2);

    let xml = std::fs::read_to_string(&report).unwrap();
    assert!(
        xml.contains(r#"<testsuites name="liminal" tests="1" failures="1""#),
        "{xml}"
    );
    assert!(xml.contains("failedAtStep=1"), "{xml}");
    assert!(xml.contains("actualValue: 65"), "{xml}");
}

#[test]
fn run_json_は単一実行でレスポンスを丸出しする() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(pass_body("Combat/Dies"));
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "run",
            "Combat/Dies",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"))
        .stdout(predicate::str::contains("PASS").not());
}

#[test]
fn run_json_は複数実行で集計付きにする() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/scenarios");
        then.status(200).body(SCENARIOS_BODY);
    });
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/scenarios/run");
        then.status(200).body(pass_body("ignored"));
    });

    cmd()
        .args([
            "--base-url",
            &server.base_url(),
            "--json",
            "run",
            "Battle/*",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 2"))
        .stdout(predicate::str::contains("\"passed\": 2"))
        // label がレスポンスの path を上書きする
        .stdout(predicate::str::contains("Battle/Alpha"));
}
