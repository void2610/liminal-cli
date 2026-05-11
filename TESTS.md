# `liminal` Rust 実装 TDD テスト項目

各フェーズで **動く CLI** を 1 機能ずつ増やしていく方式。
フェーズ N が終われば「`liminal --base-url ... health`」のように手で動かせる状態になる。
次のフェーズは前フェーズのテストを **緑のまま** 維持しながら新テストを足していく。

## 進め方

1. フェーズ冒頭の「できるようになること」を読み、まず**手で叩けるコマンド例**を確認。
2. そのフェーズの全テストを赤で書く。
3. 1 つずつ緑にしながら最低限の実装を入れる。
4. フェーズ末尾の「リファクタの目安」で内部構造を整える。
5. 次フェーズへ。

## 全体ロードマップ

| Phase | 完了時に動くこと | 主な追加 |
|---|---|---|
| 1 | `liminal --base-url URL health` | HTTP 1 本 + 出力整形 + `--json` |
| 2 | `liminal --base-url URL commands` 等 read-only 全部 | 認証 + 一覧系 4 コマンド |
| 3 | `liminal --base-url URL exec Foo/Bar v=1` | POST + exit code 2 |
| 4 | `liminal health` (URL 省略) | preferred port + DEFAULT_PORTS スキャン |
| 5 | 起動が早い `liminal health` | `~/.liminal-palette/ports.json` キャッシュ |
| 6 | 複数 Unity 同時起動でも 1 つに絞れる | `--project` / `--mode` + ambiguity エラー |
| 7 | `liminal project set-port 7613` | `ProjectSettings/LiminalPalette.json` 編集 |
| 8 | `liminal init` / `liminal doctor` | onboarding + 診断 |
| 9 | `liminal run Foo/Bar` | シナリオ単発実行 |
| 10 | `liminal run 'Battle/*'` | glob 展開 + 集計 |
| 11 | `liminal run --steps file --report x.xml` | ad-hoc + JUnit XML |

---

## Phase 1 — Hello, server

**できるようになること**:
```bash
liminal --base-url http://127.0.0.1:7610 health
liminal --base-url http://127.0.0.1:7610 health --json
```

サーバが立っていれば status / version / mode / projectName / projectPath / commandCount を表示。

**実装するもの**:
- `clap` で `liminal <subcmd> --base-url URL [--json]` の最小パース
- HTTP GET 1 本 (`Accept: application/json`、10s timeout)
- レスポンス JSON → struct → 整形出力
- ANSI カラー (`green` / `dim` / `bold`) + `NO_COLOR` / 非 TTY 抑制

**新規テスト**:

- [ ] HTTP GET が `Accept: application/json` ヘッダ付きで送られる
- [ ] 200 + JSON object → struct にデコードできる
- [ ] 200 + 空 body → エラーで止まらず空表示
- [ ] 接続失敗 → exit 1、stderr に赤エラー
- [ ] 200 でフィールド全部揃う → 全行表示
- [ ] `mode` / `projectName` / `projectPath` が空文字列 → `(unknown)` を dim
- [ ] `--json` → `to_string_pretty` 相当の整形 JSON を stdout に
- [ ] `--json` 時にカラーコードが混ざらない
- [ ] `NO_COLOR=1` でカラーコードが出ない
- [ ] 非 TTY (`isatty=false`) でカラーコードが出ない
- [ ] サブコマンド未指定 → clap がエラーで終了

**リファクタの目安**: HTTP / 出力 / 引数パースの 3 モジュールに分離。HTTP は trait 抽象化して
次フェーズ以降の wiremock テストに備える。

---

## Phase 2 — 認証 + 読み取り系コマンド

**できるようになること**:
```bash
liminal --base-url http://127.0.0.1:7610 --token "$(cat ~/.liminal-palette/token)" commands
liminal --base-url http://127.0.0.1:7610 commands --filter Player/
liminal --base-url ... state
liminal --base-url ... state Player/Health
liminal --base-url ... scenarios
liminal --base-url ... logs --limit 10
```

`~/.liminal-palette/token` / `$LP_TOKEN` も自動で読む。

**実装するもの**:
- トークン解決 (`--token` > `$LP_TOKEN` > `~/.liminal-palette/token`)
- `Authorization: Bearer ...` ヘッダ
- `GET /commands`, `/state`, `/state?path=`, `/scenarios`, `/logs?limit=`
- 各コマンドのテキスト + `--json` 出力
- HTTP エラーマッピング (`HTTP {status}: {body.error or raw}` で exit 1)

**新規テスト**:

トークン解決:
- [ ] `--token "abc"` が最優先 (env / file は無視)
- [ ] `$LP_TOKEN="xyz"` が file より優先
- [ ] `$LP_TOKEN="   "` (空白のみ) → file にフォールバック
- [ ] file 中身 `"abc\n"` → strip して `"abc"`
- [ ] file 中身が空白のみ → 「未設定」扱い
- [ ] file 不在 → 認証必須コマンドで exit 1 + 「トークンが見つかりません」

`commands`:
- [ ] 5 件取得 → 5 行表示 + `total: 5`
- [ ] `--filter Player/` で prefix 一致のみ
- [ ] フィルタ後 0 件 → `(コマンドなし)` dim
- [ ] パラメータ表示 `(name:Type, ...)` を dim
- [ ] パス幅は `min(max, 60)` でカラム揃え
- [ ] `--json` → `{"commands": [...]}` (フィルタ後)

`state`:
- [ ] PATH 指定 → `?path=...` URL エンコード付き
- [ ] PATH 省略 → 全件
- [ ] 単一: `path` / `value` / `type` 表示
- [ ] 全件: `instanceResolved=true` は `●` 緑、`false` は `○` 黄
- [ ] `value` が JSON null → 文字列 `"null"` を表示
- [ ] 0 件 → `(state なし)` dim
- [ ] `--json` 透過

`scenarios`:
- [ ] 一覧表示 + `total: N`
- [ ] `stepCount = -1` → `?` 表示
- [ ] 0 件 → `(scenario なし)`
- [ ] `--json` 透過

`logs`:
- [ ] `--limit 10` → `?limit=10`
- [ ] `--limit` 既定 20
- [ ] success=true → `✓` 緑、false → `✗` 赤 + `error: ...`
- [ ] `args` あり → `args: k=v, ...` dim
- [ ] `value` 非 null → `value: ...` dim
- [ ] 末尾 `shown N / total M`
- [ ] `--json` 透過

エラーマッピング:
- [ ] 401 + `{"error":"Unauthorized"}` → exit 1 + `HTTP 401: Unauthorized`
- [ ] 404 + `{"error":"..."}` → exit 1
- [ ] 500 + 生文字列 → exit 1 + 生文字列をそのまま表示
- [ ] 接続失敗 → exit 1 + 赤エラー

---

## Phase 3 — `exec` (POST と exit code 2)

**できるようになること**:
```bash
liminal --base-url ... exec Player/Health/Set value=100
liminal --base-url ... exec Editor/Console/Clear
echo $?  # 成功なら 0、success:false なら 2
```

**実装するもの**:
- `key=value` 引数パース (値は **常に文字列**、`split_once('=')`)
- `POST /api/v1/execute` (Content-Type: application/json)
- success=true/false で出力分岐
- logs 配列のレンダリング (Log/Warning/Error で色分け)
- **exit code 2** の特殊ルール (`--json` でも維持)

**新規テスト**:

引数:
- [ ] `value=100` → body `{"args":{"value":"100"}}`
- [ ] 引数 0 個 → body `{"args":{}}`
- [ ] `path=foo=bar` → `{"path":"foo=bar"}` (split は最初の `=` で 1 回)
- [ ] `novalue` (= 無し) → exit 1
- [ ] 値は数値化されず文字列のまま (`"100"` であって `100` ではない)

POST 構築:
- [ ] `Content-Type: application/json` 付き
- [ ] body は `{"path":"...","args":{...}}` の形

出力 (text):
- [ ] success=true → `success (1.07 ms)` 緑 + `value : ...`
- [ ] success=false → `failed` 赤 + `error : ...` + `type : ...` + stackTrace dim
- [ ] `value=null` → value 行を出さない
- [ ] logs 0 件 → logs ブロック自体出さない
- [ ] logs[].type=Log → dim、Warning → 黄、Error → 赤

Exit code:
- [ ] success=true → exit 0
- [ ] success=false → exit 2
- [ ] success=false + `--json` → JSON 出力 + exit 2 (両立)
- [ ] HTTP 500 → exit 1 (success フィールドに到達しない)
- [ ] 接続失敗 → exit 1

---

## Phase 4 — 自動 discovery (`--base-url` 不要に)

**できるようになること**:
```bash
liminal health           # URL なしで OK
liminal commands
liminal exec Foo/Bar
```

cwd が Unity プロジェクトなら `ProjectSettings/LiminalPalette.json` の preferred port を最優先で probe。
無ければ `7610..=7615` をスキャン。

**実装するもの**:
- cwd → 親方向に `ProjectSettings/ProjectVersion.txt` を探す project 検出
- `LiminalPalette.json` 読み (port / runtimePort、型違い・範囲外は無視)
- `probe_port` (`/health` を 0.4s タイムアウトで GET、JSON object のみ採用)
- candidate_ports: preferred → 隣接 5 個 → DEFAULT_PORTS、重複除外
- 全候補を probe → alive 0 / 1 / 多 で分岐 (このフェーズでは複数プロジェクト未対応 = 多なら fatal)

**新規テスト**:

cwd 検出:
- [ ] cwd 直下に `ProjectSettings/ProjectVersion.txt` → cwd を返す
- [ ] 親階層にある → その親を返す
- [ ] root まで見つからない → `None` (このとき DEFAULT_PORTS のみ使う)
- [ ] cwd が削除済み (OSError) → `None` (panic しない)

Project config 読み:
- [ ] ファイル不在 → `{editor:None, runtime:None}`
- [ ] `{"port":7613,"runtimePort":7700}` → `{Some(7613),Some(7700)}`
- [ ] `{"port":"7613"}` (型違い) → 両方 None
- [ ] `{"port":0}` / `{"port":65536}` → 範囲外で None
- [ ] 壊れた JSON → 両方 None
- [ ] 未知フィールド共存 → 既知フィールドは読める

`probe_port`:
- [ ] 200 + JSON object → `Some(dict)`
- [ ] 200 + JSON array → `None` (object 以外は捨てる)
- [ ] 200 + 非 JSON → `None`
- [ ] 4xx / 5xx → `None`
- [ ] connection refused → `None` (例外を表に出さない)
- [ ] 0.4s 以内に応答無し → `None` (タイムアウトが効く回帰テスト)

candidate_ports:
- [ ] preferred=None, cache=None → `[7610..7615]` だけ
- [ ] `preferred.editor=7613` → `[7613..7618, 7610..7612]` (重複除外で 9 個)
- [ ] `preferred.editor=65535` → `[65535, 7610..7615]` (overflow しない)

Discovery 統合:
- [ ] alive 0 → exit 1 + `Liminal Palette サーバーが見つかりません (試したポート: ...)`
- [ ] alive 1 → 採用、`base_url = http://127.0.0.1:{port}`
- [ ] alive 2 (Phase 4 では未対応) → exit 1 + `複数の Unity プロジェクトが起動中です`
- [ ] preferred port が立っていれば DEFAULT_PORTS まで probe しない (リクエスト数を assert)
- [ ] `--port N` → そのポート 1 個だけ probe

---

## Phase 5 — ポートキャッシュで起動高速化

**できるようになること**:
- 2 回目以降の `liminal health` が 1 リクエストで返る (前回成功ポートを最優先)。

**実装するもの**:
- `~/.liminal-palette/ports.json` v2 形式の load / save
- alive ヒット時に cache へ書き戻し
- discovery で「target あり + cache hit → 早出し probe → 一致すれば即採用」
- バージョン違い・壊れた JSON はサイレントに空キャッシュ化

**新規テスト**:

Load:
- [ ] ファイル不在 → `{version:2, projects:{}}`
- [ ] `version=1` → 空キャッシュ (silent reset)
- [ ] `version` 欠落 → 空
- [ ] 壊れた JSON → 空
- [ ] `projects` が array (型違い) → `{}` に補正
- [ ] 正常な v2 → roundtrip

Save:
- [ ] 親ディレクトリ不在 → `mkdir_p` してから書く
- [ ] indent=2 + 非 ASCII (日本語 projectName) がそのまま (`\uXXXX` ではない)
- [ ] 書けない (権限なし) → silent (panic しない)

`record_cache`:
- [ ] `info.projectPath` 欠落 → no-op
- [ ] `info.mode` 欠落 → `editor` として記録 (古いサーバ互換)
- [ ] 既存 entry の他 mode を保持しつつ追記
- [ ] `projectName` を更新

Discovery 早出し:
- [ ] target あり + cache hit + `/health` で一致 → 1 リクエストで採用、他を probe しない
- [ ] target あり + cache hit だが `/health` で不一致 → 通常 probe フェーズに進む
- [ ] cache に項目あるが応答無し → 通常 probe で alive を再構築
- [ ] 採用ポートが cache に書き戻される

---

## Phase 6 — 複数プロジェクト / Editor + Play Mode 同時起動

**できるようになること**:
```bash
liminal --project MyGame health
liminal --mode editor state
liminal --mode runtime state
LP_PROJECT=/path/to/proj liminal commands
```

複数 Unity Editor / Play Mode が同時に立ってても、target を 1 つに絞れる。

**実装するもの**:
- `--project` / `$LP_PROJECT` / cwd の優先順位で target 解決
- `--mode editor|runtime`
- `matches_project` / `matches_mode` (古いサーバ互換含む)
- candidate_ports に **mode 別 seed 順** を導入
- ambiguity 時のヒント文言分岐 (`--mode` / `--project` / `--port`)

**新規テスト**:

target 解決:
- [ ] `--project` 最優先
- [ ] `--project` がパスとして存在 + dir → 絶対パス化
- [ ] `--project` がパスとして無効 → 名前として保持
- [ ] `$LP_PROJECT` が次
- [ ] cwd 検出が最後

`matches_project`:
- [ ] `target=None` → true
- [ ] `info.projectPath == target` → true
- [ ] `info.projectName == target` → true
- [ ] `target` を canonicalize した結果が projectPath と一致 → true
- [ ] target が存在しないパス → エラー無く false

`matches_mode`:
- [ ] `mode=None` → true
- [ ] `info.mode = "editor"` + `mode=editor` → true
- [ ] `info.mode` 欠落 + `mode=editor` → true (古いサーバ互換)
- [ ] `info.mode` 欠落 + `mode=runtime` → false
- [ ] `info.mode = "unknown"` + `mode=editor` → true

candidate_ports (mode 入り):
- [ ] `mode=editor` + `preferred.editor=7613` → editor seed のみ
- [ ] `mode=runtime` + `runtime=7700` + `editor=7613` → `7700..` → `7613..` の順 (runtime 優先 + editor フォールバック)
- [ ] `mode=runtime` + runtime preferred なし → editor フォールバックのみ
- [ ] cache に `runtime=7800` あり、`mode=editor` 指定 → 7800 は **含まれない**

Discovery (alive 複数):
- [ ] alive 2 + 同 path / 別 mode + `--mode` なし → exit 1、ヒント `--mode editor|runtime`
- [ ] alive 2 + 別 path + フラグなし → exit 1、ヒント `--project (または --mode)`
- [ ] alive 2 + 同 path + 同 mode (異常) → exit 1、ヒント `--port`
- [ ] alive 2 + `--mode editor` で 1 件 → 採用
- [ ] alive 2 + `--project NAME` で 1 件 → 採用
- [ ] target 指定 + 一致 0 → exit 1 + `指定のプロジェクト '...' に一致する Unity サーバーが見つかりません` + 生存リスト
- [ ] mode 指定 + 一致 0 → exit 1 + `mode=... の Unity サーバーが生存していません` + 生存リスト

---

## Phase 7 — `project show` / `set-port` / `unset-port`

**できるようになること**:
```bash
liminal project show
liminal project set-port 7613
liminal project set-port --runtime 7700
liminal project unset-port
liminal project unset-port --runtime
```

cwd の Unity プロジェクトの `ProjectSettings/LiminalPalette.json` を表示・編集。

**実装するもの**:
- 「cwd が Unity プロジェクト配下でないと fatal」共通ヘルパ
- `project show` (config 表示 + live probe で自プロジェクト listener を表示)
- `project set-port` (新規 / 既存マージ、`$schema` を先頭に保つ、末尾改行 1 個)
- `project unset-port` (フィールド削除、$schema 以外残らなければファイル削除)
- 書き込みは寛容 (壊れた JSON は警告して上書き)、削除は厳格 (壊れた JSON は fatal)

**新規テスト**:

`show`:
- [ ] cwd が Unity 外 → fatal exit 1
- [ ] config 不在 → 「(no LiminalPalette.json — IpcSettings.DefaultPort=7610 にフォールバック)」
- [ ] config あり → port / runtimePort + raw 全行 dim
- [ ] live listeners: ヒット 0 → 「(no listener for this project ...)」
- [ ] live listeners: preferred と一致するなら `(matches port)` / `(matches runtimePort)` / `(matches port, runtimePort unset)`

`set-port`:
- [ ] PORT が `0` → fatal `port は 1..65535 の範囲で指定してください`
- [ ] PORT が `65536` → fatal
- [ ] 新規ファイル → `$schema` + `port` の 2 キー、末尾改行 1 個、indent=2
- [ ] 既存に `$schema` あり → 値を保持 (canonical で上書きしない)
- [ ] 既存に `$schema` なし → canonical URL を追加 + dim メッセージ
- [ ] 既存の他キー (`runtimePort`) は保持しつつ `port` を追記 / 更新
- [ ] `$schema` が常に **先頭** に出る
- [ ] `--runtime` フラグ → `runtimePort` フィールドに書く
- [ ] 親ディレクトリ不在 → `mkdir_p`
- [ ] 既存 JSON が壊れている → 警告 + 上書き (寛容)

`unset-port`:
- [ ] ファイル不在 → exit 0 + 「何もしません」 (no-op)
- [ ] JSON パース失敗 → fatal exit 1 + 修正案メッセージ (寛容にしない)
- [ ] JSON が object でない → fatal
- [ ] 該当フィールドが元々無い → 「元々設定されていません」
- [ ] フィールド削除後 user 値が残る → ファイル更新メッセージ
- [ ] フィールド削除後 `$schema` のみ残る → **ファイル削除**メッセージ
- [ ] `--runtime` フラグ → `runtimePort` のみ削除

---

## Phase 8 — `init` (onboarding) と `doctor` (診断)

**できるようになること**:
```bash
liminal init                                   # 状態確認だけ
liminal init --port 7613 --runtime-port 7700   # 同時に書き込み
liminal doctor
liminal doctor --prune-stale
```

新規プロジェクトの onboarding と環境診断。サーバ無しで動く (live probe は best-effort)。

**実装するもの**:
- `init`: Project / Project config / Token / CLI / AI Skills / Live check の 6 セクション
- `init --port` / `--runtime-port` で書き込み (フラグ無しは **副作用ゼロ**)
- `doctor`: Token / Project detection / Port cache / Live probe / Resolution の 5 セクション
- `doctor --prune-stale` で probe しても応答が無い cache entry を削除
- `doctor` は **常に exit 0**

**新規テスト**:

`init`:
- [ ] cwd が Unity 外 → fatal exit 1
- [ ] フラグなし → ファイル作成しない (実行前後で inode/mtime 不変を assert)
- [ ] `--port 7613` → ファイル作成 + `set port = 7613`
- [ ] `--runtime-port 7700` → `runtimePort` 設定
- [ ] 両フラグ → 両方書き込み
- [ ] `--port 0` → fatal `--port は 1..65535`
- [ ] config 既存 + `$schema` 無し + フラグ無し → 黄色ヒント `liminal init --port N で再書き込み`
- [ ] Token: ファイル不在 → `missing` 黄
- [ ] Token: 空ファイル → `empty` 黄
- [ ] Token: 中身あり → `exists` 緑 + 文字数
- [ ] AI Skills: `.claude/skills/liminal-foo` 1 個 → `installed 1 skill(s)` 緑
- [ ] AI Skills: `lp-foo` (legacy) 検出 → 黄色警告
- [ ] AI Skills: ディレクトリ不在 → dim メッセージ
- [ ] CLI: バイナリ絶対パス + `ln -sf ... ~/.local/bin/liminal` ヒント
- [ ] Live check: probe ヒットなら `● reachable on http://...` 緑、無ければ dim
- [ ] 末尾 `init complete` 緑

`doctor`:
- [ ] **どんな状態でも exit 0**
- [ ] Token セクションが Phase 2 の解決ロジックと一致 (env > file)
- [ ] Project detection: cwd / `--project` / `$LP_PROJECT` / resolved target / `--mode` を表示
- [ ] preferred port (port / runtimePort) を表示、未設定は dim
- [ ] Port cache 全 entry を `port {p} [{mode}] {name} {path}` 列挙
- [ ] cache 空 → `(empty)`
- [ ] Live probe: 候補ポートを列挙、結果を `●` 緑 / `○` dim
- [ ] Resolution: selected / ambiguous / no match のメッセージが Phase 6 と整合
- [ ] `--prune-stale`: probe 対象に含まれていて応答無しの cache entry を削除
- [ ] `--prune-stale`: 削除件数を `pruned N stale cache entry/entries` で表示
- [ ] `--prune-stale` 未指定: cache を一切削除しない

---

## Phase 9 — `run` (シナリオ単発)

**できるようになること**:
```bash
liminal run Combat/EnemyTakesDamage
echo $?  # PASS なら 0、FAIL なら 2
```

**実装するもの**:
- `POST /api/v1/scenarios/run` body=`{"path":"..."}`
- 単一シナリオの詳細レンダリング (各 step を `kind` 別に整形)
- exit code: PASS → 0、FAIL → 2

**新規テスト**:

リクエスト:
- [ ] `run Foo/Bar` → POST body `{"path":"Foo/Bar"}`
- [ ] 引数なし → fatal `path か --steps を指定`

出力 (text):
- [ ] PASS → `PASS Foo/Bar (12.5 ms)` 緑
- [ ] FAIL → `FAIL Foo/Bar (12.5 ms)` 赤 + `failedAtStep: N`
- [ ] 各 step: `✓ [i] Kind extra (Nms)` 形式
- [ ] `kind=Command` → `extra = commandPath`
- [ ] `kind=AssertEquals` 失敗 → `actual=X  <error 赤>`
- [ ] `kind=AssertEquals` 成功 → `actual=X` のみ
- [ ] `kind=AssertNotEquals` も同様

出力 (json):
- [ ] レスポンス body をそのまま整形出力

Exit code:
- [ ] PASS → exit 0
- [ ] FAIL → exit 2
- [ ] FAIL + `--json` → JSON 出力 + exit 2
- [ ] HTTP 500 → exit 1
- [ ] HTTP 409 (alreadyRunning) → exit 1
- [ ] HTTP 429 (rate limit) → exit 1

---

## Phase 10 — `run` の glob 展開

**できるようになること**:
```bash
liminal run 'Battle/*'        # シェル展開を防ぐためクォート必須
liminal run 'Combat/Enemy*'
```

`/api/v1/scenarios` を引いて Python `fnmatch.fnmatchcase` 互換でフィルタ → 順次実行 → 集計。

**実装するもの**:
- glob メタ文字 (`*` / `?` / `[`) 検出
- Python `fnmatch.fnmatchcase` 互換のマッチャ (`*` が `/` を跨ぐ点が `globset` と違う)
- マッチした path をソートして順次 POST
- 集計表示 (text / json) + exit code は「1 つでも fail なら 2」

**新規テスト**:

glob マッチャ:
- [ ] `*` が空文字列にマッチ
- [ ] `*` が `/` を跨ぐ (`Battle/*` が `Battle/Repro/X` にヒット) — `globset` との違いの回帰テスト
- [ ] `?` が任意の 1 文字 (`/` 含む) にマッチ
- [ ] `[abc]` が a/b/c にマッチ、d にマッチしない
- [ ] `[!abc]` (否定) が d にマッチ
- [ ] `[a-c]` のレンジ
- [ ] 大文字小文字は区別 (`fnmatchcase`)
- [ ] エスケープなし (`\` を特別扱いしない)
- [ ] リテラルのみ → 完全一致のみ true

glob モード判定:
- [ ] `Battle/Plain` → glob モードに入らない (通常実行)
- [ ] `Battle/*` / `Battle/?` / `Battle/[A-Z]` → glob モード

実行統合:
- [ ] `/scenarios` を 1 回引いてフィルタ
- [ ] マッチ 0 件 → fatal `glob '...' に一致するシナリオがありません`
- [ ] マッチ複数 → ソートして順次 POST
- [ ] glob にヒットしないものは実行されない (POST 数を assert)

出力 (glob・text):
- [ ] 各 scenario 1 行: `✓ Foo/Bar  (12 ms)` または `✗ Foo/Bar  (12 ms)  failedAtStep=N`
- [ ] FAIL の次行に `      <error>` 赤
- [ ] サマリ `PASS|FAIL  N scenarios, P passed, F failed  (T ms total)`
- [ ] 全 pass で `PASS` 緑、1 つでも fail で `FAIL` 赤
- [ ] label の幅は `min(max, 60)` で揃う

出力 (glob・json):
- [ ] payload `{scenarios:[...], total, passed, failed}`
- [ ] 各 entry の `path` は **label** で resp の path を上書き (ad-hoc 時の null 対策)

Exit code:
- [ ] glob 全 pass → exit 0
- [ ] glob 1 件以上 fail → exit 2
- [ ] glob 展開時の HTTP 500 → exit 1

---

## Phase 11 — `run --steps` (ad-hoc) と `--report` (JUnit XML)

**できるようになること**:
```bash
liminal run --steps steps.json
liminal run --steps -          # stdin から
liminal run 'Battle/**' --report reports/liminal.xml
```

**実装するもの**:
- `--steps FILE_OR_DASH` (`-` は stdin、それ以外はファイル)
- ad-hoc body builder (JSON 配列 / `{"steps":[...]}` の両方を受ける)
- `--report PATH` で JUnit XML 書き出し (親 dir 自動作成)
- XML エスケープ (`&` `<` `>` `"`、`'` はエスケープしない)

**新規テスト**:

入力モード排他:
- [ ] `run Foo --steps file` → fatal `path と --steps は同時に指定できません`
- [ ] `run --steps file` (PATH 無し) → OK
- [ ] `run` (両方無し) → fatal

ad-hoc body builder:
- [ ] JSON 配列 → body `{"steps":[...]}`
- [ ] `{"steps":[...]}` → そのまま
- [ ] `{"foo":1}` (steps 無し) → fatal
- [ ] 文字列 / 数値 → fatal
- [ ] パース失敗 → fatal
- [ ] `--steps -` → stdin から読む
- [ ] ファイル不在 → fatal

XML エスケープ:
- [ ] `&` → `&amp;` (二重エスケープしない: `&lt;` 入力 → `&amp;lt;`)
- [ ] `<` → `&lt;`、`>` → `&gt;`、`"` → `&quot;`
- [ ] `'` は **エスケープしない**

JUnit XML 生成:
- [ ] 全 pass: `<testcase name="..." time="0.012"/>` のみ + `failures="0"`
- [ ] failure 1 件: `<failure message="..." >...</failure>`
- [ ] `failedAtStep` が範囲内 → message に `failedAtStep=N — Kind — error` (` — ` 区切り)
- [ ] `failedAtStep` 範囲外 + `resp.error` あり → body に `resp.error` 1 行
- [ ] `failedAtStep` 範囲外 + `resp.error` なし → body 空
- [ ] body 行: `step[N] Kind` / `  actualValue: ...` (非 null) / `  expected: ...` (非 null) / `  error: ...`
- [ ] `time` は `{:.3}` 秒
- [ ] testsuites > testsuite で 2 重に囲む、`name="liminal"` 固定
- [ ] 末尾改行 1 個
- [ ] `name` 内の `<` がエスケープされる (XML injection 防止)

`--report` 統合:
- [ ] PATH 親ディレクトリ不在 → `mkdir_p`
- [ ] 書き込み失敗 (権限なし) → fatal exit 1
- [ ] テキストモードで書いた場合 stderr (or stdout) に `JUnit report: <path>` を dim
- [ ] `--json` と併用しても XML 書き込みは行う

---

## 仕上げ: e2e と総合

ここまでで全機能が揃う。最後に「ヘルプ表示・引数パース・ファイル副作用なし」を整理。

**仕上げテスト**:

`clap` 引数:
- [ ] `liminal --help` → exit 0、サブコマンド一覧
- [ ] `liminal exec --help` → サブコマンドヘルプ
- [ ] サブコマンドなし → clap がエラー
- [ ] 未知のサブコマンド → エラー
- [ ] グローバルフラグはサブコマンド前後どちらでも置ける (`liminal --json health` / `liminal health --json`)
- [ ] `--mode foo` (enum 違反) → clap エラー

副作用:
- [ ] 通常コマンド (`health` / `exec` / `commands` / 他) は cache 以外を書かない
- [ ] cache の更新は alive 探索の副作用としてのみ
- [ ] `init` フラグなしは ProjectSettings 配下を一切書かない

Exit code 一覧 (回帰):

| 状況 | 期待 |
|---|---|
| `health` 成功 | 0 |
| `health` 接続失敗 | 1 |
| `commands` token なし | 1 |
| `exec` success=true | 0 |
| `exec` success=false | 2 |
| `exec` HTTP 500 | 1 |
| `run` 単一 PASS | 0 |
| `run` 単一 FAIL | 2 |
| `run` glob 全 PASS | 0 |
| `run` glob 1 fail | 2 |
| `run` glob 0 マッチ | 1 |
| `doctor` (alive 0) | 0 |
| `doctor` (alive 多数) | 0 |
| `init` 成功 | 0 |
| `init` cwd 非 Unity | 1 |
| `project set-port 70000` | 1 |
| 引数パース失敗 | 2 (clap 既定) |

---

## fixtures / 共通ユーティリティ

`tests/common/` に置くと毎フェーズ使い回せる:

- `mock_server.rs` — `wiremock` セットアップ + `/api/v1/health` 等の典型レスポンス
- `temp_project.rs` — `TempDir` に `ProjectSettings/ProjectVersion.txt` を作る helper
- `temp_home.rs` — `~/.liminal-palette/` を `TempDir` に向ける (env で `HOME` 切り替え)
- `fixtures/` — JSON / JUnit XML の期待値ファイル
- `assert_cmd` ベースの CLI integration helper

各フェーズの最後に「サーバ無しで `cargo run -- <subcmd>` が動く」を手で確認するのがおすすめ。
