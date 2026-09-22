# liminal

[LiminalPalette](https://github.com/void2610/liminal-palette) の HTTP IPC サーバ (`/api/v1/*`) を叩く CLI。
Unity の Editor / Play Mode で動いているコマンドを、ターミナルや AI エージェントから実行するためのシングルバイナリ。

仕様は [SPEC.md](SPEC.md)、テスト計画は [TESTS.md](TESTS.md) にある。

## インストール

```bash
cargo build --release
ln -sf "$PWD/target/release/liminal" ~/.local/bin/liminal
```

## セットアップ

Unity プロジェクトのルートで:

```bash
liminal init              # 検出状況を表示するだけ (read-only)
liminal init --port 7613  # preferred port を ProjectSettings/LiminalPalette.json に書き込む
```

トークンは `~/.liminal-palette/token` に置くか、`$LP_TOKEN` で渡す。

## 使い方

```bash
liminal health                          # サーバの生存確認
liminal commands --filter Player/       # 登録コマンド一覧
liminal exec Player/Damage amount=10    # コマンド実行 (値は文字列のまま送る)
liminal logs --limit 50                 # 実行履歴
liminal state Player/Health             # 観測フィールドの現在値
liminal scenarios                       # シナリオ一覧
liminal run Combat/EnemyDies            # シナリオ実行
liminal run 'Battle/*' --report out.xml # glob 実行 + JUnit XML 出力
liminal doctor                          # 環境診断 (常に exit 0)
```

`--json` を付けるとサーバのレスポンスを**そのまま**出すので、`jq` に流せる。
型に落とさず中継するため、サーバ側が後から増やしたフィールドもそのまま届く。

```bash
liminal --json exec Player/Position/Get | jq -r .value
```

## 接続先の決まり方

`--base-url` を渡せばそれを使う。渡さなければ以下の順にポートを探索する (詳細は SPEC §5)。

1. `ProjectSettings/LiminalPalette.json` の `port` / `runtimePort` と、その隣接 5 ポート
2. `~/.liminal-palette/ports.json` (前回接続できたポートのキャッシュ)
3. 既定の `7610..=7615`

複数の Unity が同時に起動していると、どれを指すか決まらないのでエラーになる。
`--project <名前またはパス>` か `--mode editor|runtime` で絞る。`--port N` で直接指定してもよい。

```bash
liminal --project MyGame --mode runtime health
```

## exit code

| code | 状況 |
|---|---|
| 0 | 成功 |
| 1 | 接続できない / 引数エラー / ファイル I/O エラー |
| 2 | サーバには届いたが実行が失敗した (`exec` / `run`) |

`run` を glob で回した場合、1 つでも失敗すれば 2 になる。CI から使うときはこれを見る。

## エラー表示

サーバがエラーを返した場合、ステータスと**サーバ側のメッセージ**をそのまま出す。

```
$ liminal commands
Error: HTTP 401: token が一致しません
```

## 開発

```bash
cargo test                              # unit + integration
cargo clippy --all-targets              # 警告ゼロを維持する
cargo fmt
```

テストは `assert_cmd` でバイナリを起動し、`httpmock` でサーバを模して書く。
ユーザー環境を引き継がないよう、`tests/common/mod.rs` の `cmd()` が環境変数をクリアする。
