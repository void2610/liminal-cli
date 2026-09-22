//! integration test 用の共通ヘルパ。
//! - mock サーバを立てて `--base-url` を流し込む
//! - ユーザー環境の `$LP_TOKEN` / `$HOME` を継承させない (色付けも抑制)

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use assert_cmd::Command;
use tempfile::TempDir;

/// テスト用に環境をクリアした liminal バイナリ起動コマンドを返す。
/// - `$LP_TOKEN` / `$NO_COLOR` 等を空にしてユーザー設定を継承しない
/// - `NO_COLOR=1` を明示して anstream の色出力を完全に抑制
pub fn cmd() -> Command {
    // 認証必須コマンドはトークンが無いと実行前に落ちるので、既定で渡しておく
    let mut c = cmd_without_token();
    c.env("LP_TOKEN", "test-token");
    c
}

/// トークンを一切与えない版。未設定時の挙動を試すテストで使う。
pub fn cmd_without_token() -> Command {
    let mut c = Command::cargo_bin("liminal").expect("バイナリ liminal を build できません");
    c.env_clear()
        .env("NO_COLOR", "1")
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        // HOME は必ず差し替える。env_clear() だけでは std::env::home_dir() が
        // passwd から実ホームを拾ってしまい、利用者の ~/.liminal-palette/ を
        // 読み書きしてしまう (ports.json がテストのダミーポートで汚れる)。
        .env("HOME", isolated_home());
    c
}

/// テストバイナリごとに 1 つだけ作る使い捨て HOME。
fn isolated_home() -> &'static Path {
    static HOME: OnceLock<TempDir> = OnceLock::new();
    HOME.get_or_init(|| TempDir::new().expect("TempDir を作れません"))
        .path()
}

/// Unity プロジェクトに見える一時ディレクトリを作る。
pub fn temp_project() -> TempDir {
    let tmp = TempDir::new().expect("TempDir を作れません");
    let ps = tmp.path().join("ProjectSettings");
    std::fs::create_dir_all(&ps).unwrap();
    std::fs::write(
        ps.join("ProjectVersion.txt"),
        "m_EditorVersion: 6000.0.0f1\n",
    )
    .unwrap();
    tmp
}

/// preferred port 付きの Unity プロジェクトを作る。
pub fn temp_project_with_port(port: u16) -> TempDir {
    let tmp = temp_project();
    std::fs::write(
        project_config_path(tmp.path()),
        format!("{{\"port\":{port}}}"),
    )
    .unwrap();
    tmp
}

/// `<root>/ProjectSettings/LiminalPalette.json` のパス。
pub fn project_config_path(root: &Path) -> PathBuf {
    root.join("ProjectSettings/LiminalPalette.json")
}

/// `~/.liminal-palette/` を差し替えるための一時 HOME を作る。
/// `cmd().env("HOME", home.path())` で使う。
pub fn temp_home() -> TempDir {
    let tmp = TempDir::new().expect("TempDir を作れません");
    std::fs::create_dir_all(tmp.path().join(".liminal-palette")).unwrap();
    tmp
}

/// 一時 HOME にポートキャッシュを書く。
pub fn write_port_cache(home: &Path, project_path: &str, project_name: &str, editor_port: u16) {
    std::fs::write(
        home.join(".liminal-palette/ports.json"),
        format!(
            r#"{{"version":2,"projects":{{"{project_path}":{{"projectName":"{project_name}","ports":{{"editor":{editor_port}}}}}}}}}"#
        ),
    )
    .unwrap();
}

/// 誰も listen していないポートを 1 つ得る (bind して即 drop する)。
pub fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

/// `/api/v1/health` の典型レスポンス。
pub fn health_body(mode: &str, name: &str, path: &str) -> String {
    format!(
        r#"{{"status":"ok","version":"0.2.0","mode":"{mode}","projectName":"{name}",
             "projectPath":"{path}","commandCount":1}}"#
    )
}
