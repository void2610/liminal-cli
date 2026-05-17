//! integration test 用の共通ヘルパ。
//! - mock サーバを立てて `--base-url` を流し込む
//! - ユーザー環境の `$LP_TOKEN` / `$HOME` を継承させない (色付けも抑制)

use assert_cmd::Command;

/// テスト用に環境をクリアした liminal バイナリ起動コマンドを返す。
/// - `$LP_TOKEN` / `$NO_COLOR` 等を空にしてユーザー設定を継承しない
/// - `NO_COLOR=1` を明示して anstream の色出力を完全に抑制
pub fn cmd() -> Command {
    let mut c = Command::cargo_bin("liminal").expect("バイナリ liminal を build できません");
    c.env_clear()
        .env("NO_COLOR", "1")
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    c
}
