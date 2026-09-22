//! SPEC §4.2 `init` / §4.3 `doctor` / §4.4 `project` / §9 設定ファイルの integration test

// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#![allow(non_snake_case)]

mod common;

use common::cmd;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Unity プロジェクトに見える一時ディレクトリを作る。
fn unity_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let ps = tmp.path().join("ProjectSettings");
    std::fs::create_dir_all(&ps).unwrap();
    std::fs::write(
        ps.join("ProjectVersion.txt"),
        "m_EditorVersion: 6000.0.0f1\n",
    )
    .unwrap();
    tmp
}

fn config_path(root: &Path) -> PathBuf {
    root.join("ProjectSettings/LiminalPalette.json")
}

#[test]
fn project_show_は設定と検出パスを表示する() {
    let tmp = unity_project();
    std::fs::write(config_path(tmp.path()), r#"{"port":7613}"#).unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("7613"))
        .stdout(predicate::str::contains("runtimePort : (unset)"));
}

#[test]
fn project_はUnityプロジェクト外では致命エラー() {
    let tmp = TempDir::new().unwrap();
    cmd()
        .current_dir(tmp.path())
        .args(["project", "show"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Unity プロジェクトが見つかりません",
        ));
}

#[test]
fn set_port_は_schema_を付けて書き込む() {
    let tmp = unity_project();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "set-port", "7613"])
        .assert()
        .success()
        .stdout(predicate::str::contains("port = 7613"));

    let raw = std::fs::read_to_string(config_path(tmp.path())).unwrap();
    assert!(raw.contains("$schema"), "{raw}");
    assert!(raw.contains("\"port\": 7613"), "{raw}");
    // 末尾は改行 1 個
    assert!(raw.ends_with("}\n"), "{raw:?}");
}

#[test]
fn set_port_runtime_は_runtimePort_に書く() {
    let tmp = unity_project();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "set-port", "--runtime", "7700"])
        .assert()
        .success();

    let raw = std::fs::read_to_string(config_path(tmp.path())).unwrap();
    assert!(raw.contains("\"runtimePort\": 7700"), "{raw}");
    assert!(!raw.contains("\"port\":"), "{raw}");
}

#[test]
fn set_port_は既存の未知フィールドを保持する() {
    let tmp = unity_project();
    std::fs::write(
        config_path(tmp.path()),
        r#"{"$schema":"https://example.com/x.json","extra":true}"#,
    )
    .unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "set-port", "7613"])
        .assert()
        .success();

    let raw = std::fs::read_to_string(config_path(tmp.path())).unwrap();
    assert!(raw.contains("https://example.com/x.json"), "{raw}");
    assert!(raw.contains("\"extra\": true"), "{raw}");
}

#[test]
fn set_port_は範囲外を拒否する() {
    let tmp = unity_project();
    cmd()
        .current_dir(tmp.path())
        .args(["project", "set-port", "0"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("1..=65535"));
}

#[test]
fn set_port_は壊れたJSONに寛容で上書きする() {
    let tmp = unity_project();
    std::fs::write(config_path(tmp.path()), "{broken").unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "set-port", "7613"])
        .assert()
        .success()
        .stdout(predicate::str::contains("上書き"));

    assert!(
        std::fs::read_to_string(config_path(tmp.path()))
            .unwrap()
            .contains("7613")
    );
}

#[test]
fn unset_port_は壊れたJSONを致命扱いにする() {
    let tmp = unity_project();
    std::fs::write(config_path(tmp.path()), "{broken").unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "unset-port"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("解釈できません"));

    // 上書きされていないこと
    assert_eq!(
        std::fs::read_to_string(config_path(tmp.path())).unwrap(),
        "{broken"
    );
}

#[test]
fn unset_port_で空になればファイルごと消す() {
    let tmp = unity_project();
    std::fs::write(config_path(tmp.path()), r#"{"port":7613}"#).unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "unset-port"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));

    assert!(!config_path(tmp.path()).exists());
}

#[test]
fn unset_port_で他のキーが残るなら書き戻す() {
    let tmp = unity_project();
    std::fs::write(
        config_path(tmp.path()),
        r#"{"port":7613,"runtimePort":7700}"#,
    )
    .unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["project", "unset-port"])
        .assert()
        .success()
        .stdout(predicate::str::contains("updated"));

    let raw = std::fs::read_to_string(config_path(tmp.path())).unwrap();
    assert!(!raw.contains("\"port\":"), "{raw}");
    assert!(raw.contains("\"runtimePort\": 7700"), "{raw}");
}

#[test]
fn unset_port_はファイル不在なら_no_op() {
    let tmp = unity_project();
    cmd()
        .current_dir(tmp.path())
        .args(["project", "unset-port"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no config file"));
}

#[test]
fn init_はフラグ無しなら書き込まない() {
    let tmp = unity_project();

    cmd()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("init complete"))
        .stdout(predicate::str::contains("no config file"));

    // read-only の保証
    assert!(!config_path(tmp.path()).exists());
}

#[test]
fn init_port_指定時だけ書き込む() {
    let tmp = unity_project();

    cmd()
        .current_dir(tmp.path())
        .args(["--port", "7613", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("set port = 7613"));

    assert!(
        std::fs::read_to_string(config_path(tmp.path()))
            .unwrap()
            .contains("7613")
    );
}

#[test]
fn init_runtime_port_も書ける() {
    let tmp = unity_project();

    cmd()
        .current_dir(tmp.path())
        .args(["init", "--runtime-port", "7700"])
        .assert()
        .success()
        .stdout(predicate::str::contains("set runtimePort = 7700"));
}

#[test]
fn init_はレガシーskillを警告する() {
    let tmp = unity_project();
    std::fs::create_dir_all(tmp.path().join(".claude/skills/lp-exec")).unwrap();
    std::fs::create_dir_all(tmp.path().join(".claude/skills/liminal-exec")).unwrap();

    cmd()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("legacy skill"))
        .stdout(predicate::str::contains("lp-exec"));
}

#[test]
fn doctor_は生存サーバが無くても_exit0() {
    let tmp = unity_project();
    cmd()
        .current_dir(tmp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Token"))
        .stdout(predicate::str::contains("Project detection"))
        .stdout(predicate::str::contains("Port cache"))
        .stdout(predicate::str::contains("Live probe"))
        .stdout(predicate::str::contains("Resolution"));
}

#[test]
fn doctor_は_preferred_port_を表示する() {
    let tmp = unity_project();
    std::fs::write(config_path(tmp.path()), r#"{"port":7613}"#).unwrap();

    cmd()
        .current_dir(tmp.path())
        .args(["--port", "7613", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("port=7613"));
}
