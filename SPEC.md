# `liminal` CLI 仕様書 (Rust 書き直し用)

現行の Python 実装 (`Tools~/liminal/liminal`, 1469 行) を Rust に書き直すための仕様。
本ドキュメントは「外から見える振る舞い (CLI 引数 / ファイル形式 / HTTP リクエスト / 出力 / exit code)」を
網羅することを目的とする。内部実装は自由。

## 1. 概要

LiminalPalette Unity パッケージが立てる HTTP IPC サーバ (`/api/v1/*`) を叩くシングルバイナリ CLI。

特徴:
- ローカル `127.0.0.1` 専用。`Bearer` トークン認証。
- ポート自動発見 (preferred port → キャッシュ → デフォルト `7610..=7615`)。
- 複数 Unity プロジェクト / Editor + Play Mode 同時起動を `/health` の `mode` / `projectName` / `projectPath` で識別。
- `--json` でパイプ可能な生 JSON 出力。
- `doctor` / `project` / `init` はサーバなしでも動く (ローカル状態だけを触る)。

## 2. ファイル / 環境変数 / 既定値

| 項目 | 既定値 | 用途 |
|---|---|---|
| トークンファイル | `~/.liminal-palette/token` | UTF-8、末尾改行 strip。中身が空白だけなら「未設定」扱い |
| ポートキャッシュ | `~/.liminal-palette/ports.json` | 最近成功したポートを mode 別に記憶 (後述の v2 形式) |
| プロジェクト設定 | `<project>/ProjectSettings/LiminalPalette.json` | preferred port を宣言 |
| プロジェクトマーカー | `<project>/ProjectSettings/ProjectVersion.txt` | cwd から親方向に探索して Unity プロジェクトを判定 |
| 環境変数 `LP_TOKEN` | — | トークンの上書き (ファイルより優先) |
| 環境変数 `LP_PROJECT` | — | ターゲットプロジェクトの上書き (cwd 検出より優先) |
| 環境変数 `NO_COLOR` | — | セットされていればカラー出力を抑制 |
| 候補ポート (デフォルト) | `[7610, 7611, 7612, 7613, 7614, 7615]` | 6 個 |
| HTTP タイムアウト (本リクエスト) | 10s | |
| HTTP タイムアウト (discovery 中の `/health` probe) | 0.4s | 立っていないポートに長く待たない |
| `--base-url` 指定時の `/health` probe | 2.0s | best-effort、失敗しても致命にしない |
| Project config 用 `$schema` URL | `https://raw.githubusercontent.com/void2610/liminal-palette/main/Documentation~/schemas/LiminalPalette.schema.json` | 書き出すときに自動付与 |

トークン解決の優先順位 (現状実装):
1. `--token` 明示
2. `$LP_TOKEN` (strip 後に空でなければ採用)
3. `~/.liminal-palette/token` の中身 (strip)
4. なし

ターゲットプロジェクトの解決:
1. `--project NAME_OR_PATH`
2. `$LP_PROJECT`
3. cwd から親方向に `ProjectSettings/ProjectVersion.txt` を持つ最初のディレクトリ

`--project` の値はパスとして存在し `is_dir()` なら絶対パスに正規化、そうでなければ「名前」として扱う
(後段の `/health.projectName` 一致で使う)。

## 3. グローバルオプション

すべてのサブコマンドの **前** に置く (Python 実装の `argparse` 並び順をそのまま採用)。

| オプション | 型 | 既定 | 説明 |
|---|---|---|---|
| `--base-url URL` | string | — | ベース URL を直接指定 (例: `http://127.0.0.1:7611`)。discovery をバイパス |
| `--port N` | u16 | — | ポートだけ指定。隣接探索もしない (1 ポートのみ probe) |
| `--project NAME_OR_PATH` | string | `$LP_PROJECT` / cwd | ターゲット指定 |
| `--mode editor\|runtime` | enum | (auto) | Editor 側か Play Mode (Runtime) 側か |
| `--token T` | string | file/env | Bearer トークン |
| `--json` | flag | false | JSON 出力 |

`--json` はサブコマンドより前にも後にも置けるよう、グローバルに定義しておく
(現行 Python 実装はグローバルのみ、サブコマンド側にはない)。

## 4. サブコマンド一覧

サブコマンドは **必須** (未指定はエラー)。

| サブコマンド | 認証 | サーバ必要 | 概要 |
|---|---|---|---|
| `health` | 不要 | 必要 | サーバ生存確認 |
| `init` | 不要 | 不要 (live probe は best-effort) | プロジェクト onboarding。`--port` / `--runtime-port` で書き込みもできる |
| `doctor` | 不要 | 不要 | 環境診断: token / cwd / cache / 生存ポート |
| `project show` | 不要 | 不要 (live probe は best-effort) | プロジェクト設定ファイルを表示 + live probe |
| `project set-port [--runtime] PORT` | 不要 | 不要 | preferred port を書き込み |
| `project unset-port [--runtime]` | 不要 | 不要 | preferred port を削除 |
| `commands [--filter PREFIX]` | 必要 | 必要 | 登録コマンド一覧 |
| `exec PATH [KEY=VAL]...` | 必要 | 必要 | コマンド実行 |
| `logs [--limit N]` | 必要 | 必要 | 実行履歴 |
| `state [PATH]` | 必要 | 必要 | LiminalObservableField スナップショット |
| `scenarios` | 必要 | 必要 | シナリオ一覧 |
| `run [PATH] [--steps FILE_OR_DASH] [--report PATH]` | 必要 | 必要 | シナリオ実行 (named / glob / ad-hoc) |

### 4.1 `health`

`GET /api/v1/health` (認証ヘッダなし) を叩いて結果を表示。

テキスト出力 (現行):
```
ok  http://127.0.0.1:7610
  version       : 0.4.0
  mode          : editor
  projectName   : MyGame
  projectPath   : /Users/me/dev/MyGame
  commandCount  : 356
```
`mode` / `projectName` / `projectPath` が空文字列なら `(unknown)` を dim 色で表示。

`--json` 時は body をそのまま `json.dumps(..., indent=2)` で出す。

### 4.2 `init`

引数:
- `--port N` — `port` (Editor) を書き込み (1..=65535)。
- `--runtime-port N` — `runtimePort` (Play Mode) を書き込み (1..=65535)。

副作用ゼロを保証するため、**フラグ無しの `init` は read-only**。`--port` / `--runtime-port` のいずれかが
渡された場合のみ `ProjectSettings/LiminalPalette.json` に書き込む。

出力は以下のセクション順:
1. **Project** — cwd から検出した Unity プロジェクトパス。検出失敗なら fatal (赤エラー + exit 1)。
2. **Project config** — `ProjectSettings/LiminalPalette.json` のパス + 現在値 (port / runtimePort)。
   - 書き込みが発生したら `set port = N` / `set runtimePort = N` を表示。
   - 既存に `$schema` が無かった場合は dim で「`$schema` reference を追加」と表示。
   - ファイル未作成かつフラグも無ければ「(no config file — ...)」を dim で表示。
   - フラグ無し + ファイル存在 + `$schema` 無し → 黄色で「`liminal init --port N` で再書き込みしてください」のヒント。
3. **Token** — `~/.liminal-palette/token` の有無 (exists/empty/missing)。
4. **CLI** — 自分のバイナリ絶対パス + `ln -sf <path> ~/.local/bin/liminal` のヒント。
5. **AI Skills** — `<project>/.claude/skills/` 配下に `liminal-*` ディレクトリがあるか確認、`lp-*` (legacy) があれば警告。
6. **Live check** — preferred + cached + DEFAULT のポートを順に probe して 1 つでも当たれば緑、無ければ dim メッセージ。

最後に `init complete` を緑で表示。

### 4.3 `doctor`

引数:
- `--prune-stale` — probe で応答が無かった cache エントリを削除。

セクション (現行とそろえる):
1. **Token:** `$LP_TOKEN` / `~/.liminal-palette/token` の検出。空白のみは `empty` 警告。
2. **Project detection:** `cwd`、cwd から検出した project path、`--project`、`$LP_PROJECT`、resolved target、`--mode`、preferred port (editor / runtime)。
3. **Port cache:** `~/.liminal-palette/ports.json` の全エントリを `port {p} [{mode}] {name} {path}` 形式で列挙。
4. **Live probe (...):** `_build_candidate_ports(None, preferred, mode, cache)` + cache の全 port を 1 回ずつ probe。
   応答ありは緑 `● {port} [{mode}] {name} v{ver} cmds={cnt} {path}`、無しは dim `○ {port} (no response)`。
5. **Resolution:** target / mode フィルタを適用して「selected」/「ambiguous」/「no port matches target」/「no live server」を表示。
   `--prune-stale` の場合は削除件数も表示。

`doctor` は **どんな状態でも exit 0** で抜ける (純粋な診断コマンド)。

### 4.4 `project show` / `set-port` / `unset-port`

#### show
- cwd から Unity プロジェクトを必須 (見つからなければ fatal + exit 1)。
- `ProjectSettings/LiminalPalette.json` のパス、port / runtimePort、raw 内容を dim で表示。
- Live listeners セクション: `_build_candidate_ports(None, preferred, None)` を probe してこのプロジェクト宛のものだけ表示。
  - port が preferred と一致するときに `(matches port)` / `(matches runtimePort)` / `(matches port, runtimePort unset)` のタグを付ける。

#### set-port [--runtime] PORT
- cwd Unity プロジェクト必須。
- PORT は `1..=65535`、範囲外は fatal。
- `--runtime` なら `runtimePort` フィールド、無ければ `port` フィールド。
- 書き込みは「既存 JSON を読む → `$schema` を先頭に保ったまま該当フィールドを更新 → 末尾改行付きで indent=2 JSON を書き戻す」。
- 既存に `$schema` が無ければ canonical URL を追加。
- 既存 JSON が壊れていた場合 (`set-port` は寛容): 警告メッセージを出して **上書き** する。

#### unset-port [--runtime]
- cwd Unity プロジェクト必須。
- ファイルが無ければ no-op (dim メッセージ)。
- JSON パース失敗は **致命エラー** (上書きすると意図が伝わらないため)。
- 該当フィールドを削除。`$schema` 以外のユーザーキーが何も残らなくなったらファイルごと削除して `removed` メッセージ。
  残るなら `_write_project_config` で書き戻して `updated` メッセージ。

### 4.5 `commands`

`GET /api/v1/commands` を叩いて `body.commands[]` を取得。

- `--filter PREFIX` 指定時は `path.starts_with(PREFIX)` で絞り込み。
- `--json` 時は `{"commands": [...]}` を出力 (`--filter` 適用後)。
- テキスト時:
  - パス幅は `min(max(len(path)), 60)` で揃える。
  - 1 行: `  {cyan(path.ljust(width))}  {description}{dim(" (k1:T1, k2:T2)")}`
  - 末尾に `  total: N` を dim で。
  - コマンドが 0 件なら dim `(コマンドなし)`。

### 4.6 `exec PATH [KEY=VAL]...`

- 引数を `key=value` でパースして dict にする。`=` が無ければ fatal。**値は文字列のまま** (型変換しない)。
- `POST /api/v1/execute` に `{"path": PATH, "args": {...}}` を送る。
- `--json` 時はレスポンスを丸出力。
- テキスト出力:
  ```
  success  (1.07 ms)            (or "failed")
    value : (2.00, 4.00, 6.00)
    error : <msg>               (失敗時)
    type  : <exceptionType>     (失敗時かつ非 null)
    <stackTrace>                (dim 色で複数行)

    logs (N):
      Log: ...
      Warning: ...
      Error: ...
  ```
  - `value` が `null` のときは表示しない。
  - `logs[].type` が `"Error"` なら赤、`"Warning"` なら黄、その他は dim。
- **exit code は `success` フィールドで決まる**: true → 0、false → 2。
  - これは `--json` でも同じ (JSON だけ出力した上で 2 で抜ける)。

### 4.7 `logs`

引数: `--limit N` (既定 20、サーバ側上限 200)。

- `GET /api/v1/logs?limit=N` (limit が指定されていなければクエリ無し)。
- `--json` 時はレスポンス丸出し。
- テキスト時、各 invocation を 1 行 + α で表示:
  ```
    ✓  {dim ts}  {cyan path}  ({dur}ms)
        args: k=v, k=v          (args が空でなければ)
        value: ...              (value が非 null)
        error: ...              (失敗時、赤)
  ```
  末尾に `  shown N / total M` (dim)。

### 4.8 `state [PATH]`

- `PATH` あり: `GET /api/v1/state?path=...` (URL エンコード)。レスポンス `{path, value, type}` を表示。
- `PATH` なし: `GET /api/v1/state`。レスポンス `{fields: [{path, value, type, instanceResolved}]}` を表示。
  - `instanceResolved=false` は黄色 `○` マーカー、true は緑 `●`。
  - 値が JSON `null` の表示は文字列 `"null"`。
- `--json` 時は body 丸出し。

### 4.9 `scenarios`

- `GET /api/v1/scenarios` 取得。
- `--json` で丸出し。
- テキスト: 各 scenario を `  {cyan path}  {description} [N steps]` (`stepCount = -1` のときは `?`)。末尾に `total: N`。

### 4.10 `run`

引数:
- 位置引数 `PATH` (省略可、`--steps` 使用時は省略)。
- `--steps FILE_OR_DASH` — `-` なら stdin、それ以外はファイルから JSON ステップを読む。
- `--report PATH` — JUnit XML を書き出す (CI 向け)。

実行モード:
1. **ad-hoc**: `--steps` 指定時。`PATH` 同時指定は fatal。
   - JSON は配列 `[{type:...}, ...]` または `{"steps":[...]}` を受け付ける。
   - リクエスト body は `{"steps": [...]}`。
2. **glob**: `PATH` に `*` / `?` / `[` のいずれかを含むとき。
   - `GET /api/v1/scenarios` で一覧取得 → `fnmatch.fnmatchcase` 相当のグロブで `path` をフィルタ。
   - **`*` は `/` を跨ぐ** (`Battle/*` が `Battle/Repro/X` にもヒット)。Python `fnmatch` の仕様をそのまま採用すること
     (Rust 側で実装するなら `glob` クレートではなく自前実装、または `?` = 1 文字 / `*` = 任意文字列 / `[abc]` = 文字クラスの再現)。
   - マッチ 0 件は fatal。
   - マッチした path をソートして順次 `POST /api/v1/scenarios/run`。
3. **named (単一)**: 上記以外。`POST /api/v1/scenarios/run` body=`{"path": PATH}`。

出力:
- 単一実行 (named or ad-hoc) かつ `--json`: レスポンス丸出し。
- 単一実行 かつテキスト:
  ```
  PASS  Combat/EnemyTakesDamage  (12.5 ms)        (or FAIL)
    failedAtStep: 4                                 (失敗時のみ)
    ✓ [0] Command         Enemy/Spawn        (1.2ms)
    ✗ [1] AssertEquals    actual=65  expected '70' but got '65'  (0.1ms)
  ```
  - `kind` ごとの `extra` フィールド:
    - `Command` → `commandPath`
    - `AssertEquals` / `AssertNotEquals` → `actual={actualValue}  {error?}`
    - その他 → なし
- 複数 (glob) かつ `--json`:
  ```json
  {
    "scenarios": [
      {...resp, "path": "<label>"},
      ...
    ],
    "total": N,
    "passed": P,
    "failed": F
  }
  ```
  `label` を **最後に** 書く (resp に `path` が含まれても上書きする = ad-hoc 実行時の null 対策)。
- 複数 かつテキスト:
  ```
    ✓ Combat/EnemyDies          (12 ms)
    ✗ Combat/PlayerHeals        (35 ms)  failedAtStep=1
        expected '70' but got '65'
  
  PASS  3 scenarios, 2 passed, 1 failed  (59.7 ms total)
  ```
  幅は label の `min(max, 60)`。

`--report PATH`:
- JUnit XML を書く (詳細は §7)。
- 親ディレクトリが無ければ自動作成。

`run` の exit code: 1 つでも失敗していれば 2、全部成功で 0。

## 5. Discovery アルゴリズム

`--base-url` が明示されていれば即採用 (`/health` は best-effort で取得、失敗は無視)。

そうでなければ:

1. cwd / `--project` から `<project>/ProjectSettings/LiminalPalette.json` を読む。
   - 数値で `1..=65535` のもののみ採用。型違いは無視。
2. cache (`~/.liminal-palette/ports.json`) をロード。
3. 候補ポート列 `candidate_ports` を組み立てる:
   - `--port` が明示されていれば `[port]` のみ。
   - そうでなければ:
     - seeds を mode によって決める:
       - `--mode editor`: `[preferred.editor]` (あれば)
       - `--mode runtime`: `[preferred.runtime, preferred.editor]` (あるものだけ、この順)
       - `--mode` 未指定: `[preferred.editor, preferred.runtime]`
     - 各 seed を `seed..=seed+5` に展開 (`1..=65535` の範囲チェック付き)。
     - cache から `mode_wanted` (1 個または `(editor, runtime)`) に合うすべての port を追加。
     - 最後に `DEFAULT_PORTS = [7610..7615]` を追加。
   - **すべての段階で重複は除外** (順序は最初に現れた位置)。
4. ターゲット指定がある場合: cache のうち target に一致 (projectPath 完全一致 or projectName 完全一致) するポートだけを先に試す。
   `mode` 指定があれば mode フィルタも適用。各候補を probe → ヒットしたら `/health` の projectName / projectPath / mode で target / mode 一致を再確認、合致すれば即採用 + cache 更新。
5. cache 早出しで決まらなければ candidate_ports 全部に対して 0.4s タイムアウトで probe (2xx + JSON object のみ受け付け)。alive list を構築。
   - 並列 probe にしてもよい (順序は保たれていなくてもよい)。alive 順序は表示用に元のポート順を保持する。
6. alive 0 個 → fatal: `Liminal Palette サーバーが見つかりません (試したポート: ...)`。exit 1。
7. alive を target / mode でフィルタ:
   - target 指定あり + 0 件 → fatal `指定のプロジェクト '...' に一致する Unity サーバーが見つかりません。\n生存中:\n<list>`。
   - mode 指定あり + 0 件 → fatal `mode=... の Unity サーバーが生存していません。\n生存中:\n<list>`。
   - target/mode 指定なし + alive 1 個 → 採用。
   - target/mode 指定なし + alive 2 個以上 → fatal `複数の Unity プロジェクトが起動中です。--project / --mode ...`。
   - フィルタ後 1 件 → 採用。
   - フィルタ後 2 件以上 → fatal。ヒントは判別ロジック:
     - 同じ projectPath で mode だけ違う → `--mode editor|runtime`
     - projectPath が違う → `--project (または --mode)`
     - それ以外 → `--port`
8. 採用したら cache に書き戻して `(base_url, /health.body)` を返す。

`/health` の dict にマッチさせるヘルパ:
- `_matches_project(info, target)`:
  - `target` is `None` → true。
  - `info.projectPath == target` または `info.projectName == target` → true。
  - `target` をパスとして `canonicalize` できれば、`info.projectPath` とも比較。
- `_matches_mode(info, mode)`:
  - `mode` is `None` → true。
  - `info.mode` が `editor` / `runtime` のどちらでもなければ `editor` とみなす (古いサーバ互換)。

## 6. HTTP API

すべて `127.0.0.1:<port>` 限定。`Accept: application/json` を常に送る。
body 送信時は `Content-Type: application/json`、`json.dumps(body, ensure_ascii=False)` 相当の UTF-8。

| Method | Path | 認証 | リクエスト | レスポンス (200) |
|---|---|---|---|---|
| GET | `/api/v1/health` | 不要 | — | `{status, version, mode, projectName, projectPath, commandCount}` |
| GET | `/api/v1/commands` | Bearer | — | `{commands: [{path, name, category, description, isAsync, returnType, aliases, parameters:[{name,type,position,hasDefault,default,description,choices}]}]}` |
| POST | `/api/v1/execute` | Bearer | `{path, args:{<string>:<string>}}` | `{success, value, error, exceptionType, stackTrace, durationMs, logs:[{type,message,stackTrace,timestamp}]}` |
| GET | `/api/v1/logs?limit=N` | Bearer | — | `{invocations:[{path,timestamp,args,result:{...exec resp...}}], total, limit}` |
| GET | `/api/v1/state[?path=...]` | Bearer | — | 単一: `{path,value,type}`、全件: `{fields:[{path,value,type,instanceResolved}]}` |
| GET | `/api/v1/scenarios` | Bearer | — | `{scenarios:[{path,description,stepCount}]}` |
| POST | `/api/v1/scenarios/run` | Bearer | `{path}` または `{steps:[...]}` | `{success,durationMs,failedAtStep,path,alreadyRunning,steps:[{kind,success,durationMs,...}]}` |

エラーレスポンス:
- 非 2xx は body が `{"error": "<message>"}` か生文字列。
- 表示は `HTTP {status}: {body.error or stringified body}` を赤で → exit 1。
- `URLError` (ネットワーク不到達) も同じく赤で fatal、exit 1。

### `run` のステップ JSON フォーマット (ad-hoc 用、サーバが受ける形)

| `type` | 必須 | 任意 |
|---|---|---|
| `command` | `path` (str), `args` (object) | `description` |
| `wait_seconds` | `seconds` (num) | `description` |
| `wait_frames` | `frames` (int) | `description` |
| `assert_equals` | `path` (str), `expected` (str/num/bool/null) | `description` |
| `assert_not_equals` | `path` (str), `expected` | `description` |

CLI 側はバリデーションせず、JSON をそのまま中継してよい (サーバが 400 を返す)。

## 7. JUnit XML レポート

`liminal run ... --report PATH` で書き出す。

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="liminal" tests="{N}" failures="{F}" time="{total_sec:.3f}">
  <testsuite name="liminal" tests="{N}" failures="{F}" time="{total_sec:.3f}">
    <testcase name="{label}" time="{dur_sec:.3f}"/>                  <!-- pass -->
    <testcase name="{label}" time="{dur_sec:.3f}">
      <failure message="{escaped msg}">{escaped body}</failure>
    </testcase>
  </testsuite>
</testsuites>
```

- `time` 値は秒、`durationMs / 1000.0` を `{:.3f}` で。
- `failure.message`:
  - `failedAtStep={idx}` を最初に。
  - 失敗ステップが取れれば `kind`、`error` (あれば) を追加。
  - 全部を ` — ` で結合。
- `failure` body:
  - `step[{idx}] {kind}`
  - `  actualValue: {actual}` (非 null)
  - `  expected: {expected}` (非 null)
  - `  error: {err}` (あれば)
  - 上記が取れない場合は `resp.error` を 1 行。
  - `\n` で結合。
- XML エスケープ: `&` `<` `>` `"` の 4 つ。`'` はエスケープしない。
- 親ディレクトリは `mkdir_p`。

## 8. キャッシュファイル (`~/.liminal-palette/ports.json`)

```json
{
  "version": 2,
  "projects": {
    "<absolute projectPath>": {
      "projectName": "<string>",
      "ports": {
        "editor": 7610,
        "runtime": 7611
      }
    }
  }
}
```

- `version != 2` のものはサイレントに破棄 (空状態として扱う)。
- 読みも書きも **ベストエフォート**。OSError / JSON エラーは全部握りつぶす。
- ファイル親 dir は書き込み時に `mkdir_p`。
- 書き込み形式は `indent=2`, `ensure_ascii=false`。
- 書き戻し条件: `_record_cache` で alive ヒットしたとき & `--prune-stale` で削除したとき。
- mode が読み取れない `/health` 応答は `editor` とみなして記録する (古いサーバ互換)。

## 9. プロジェクト設定ファイル (`<project>/ProjectSettings/LiminalPalette.json`)

JSON Schema: `Documentation~/schemas/LiminalPalette.schema.json` (本リポジトリ同梱)。

```json
{
  "$schema": "<canonical URL>",
  "port": 7613,
  "runtimePort": 7700
}
```

- `port` / `runtimePort` は integer `1..=65535`。型違い・範囲外は **無視** (読み)。
- 書くときは `$schema` を必ず先頭に。既存値があれば保持、なければ canonical URL を入れる。
- 末尾に **改行 1 個** を入れて書き出す (Python 実装は `json.dumps(...) + "\n"`)。
- ファイル全体が `$schema` 以外空になったら `unset-port` がファイルごと削除する。

## 10. 出力ヘルパ (色 / フォーマット)

- TTY (`stdout.is_terminal()`) で `$NO_COLOR` 未設定なら ANSI カラーを使う。それ以外は色を抑制。
- カラーコード:
  - green: `\033[32m`
  - red: `\033[31m`
  - yellow: `\033[33m`
  - cyan: `\033[36m`
  - dim: `\033[2m`
  - bold: `\033[1m`
  - reset: `\033[0m`
- JSON 出力は `serde_json::to_string_pretty` 相当 + `ensure_ascii=false` (UTF-8 そのまま)。

## 11. Exit code

| code | 状況 |
|---|---|
| 0 | 成功 |
| 1 | HTTP/ネットワークエラー、トークン未設定、引数エラー、ファイル I/O 致命エラー等の汎用「使用エラー」 |
| 2 | コマンドはサーバに届いたが `success: false` (`exec` / `run`)。`run` glob では 1 つでも失敗すれば 2 |

`doctor` は診断目的なので、常に 0 で抜ける (生存サーバ 0 でも 0)。

## 12. Rust 実装に向けたメモ (任意)

参考までに、Python 実装から見たクレート選定の自然な対応:

| Python | 想定 Rust クレート |
|---|---|
| `argparse` | `clap` (derive API が一番楽) |
| `urllib.request` + `json` | `ureq` + `serde_json`、または `reqwest` (blocking) |
| `pathlib` | `std::path` |
| `fnmatch` | 自前実装 (`*` が `/` を跨ぐ Python 仕様)、または既存クレート `globset` だと挙動が違うので注意 |
| ANSI カラー | `anstream` / `nu-ansi-term` / 手書き |
| ファイル I/O | `std::fs` |

discovery の probe は順次でも並列でもよいが、**順序**は表示やログのために保ちたいので、
並列にするなら collect 後に元の `candidate_ports` 順に並べ替えてから表示すること。

非同期は **不要**。シングルスレッド + blocking I/O で十分。

## 13. テストすべきシナリオ (回帰テスト用)

最低限カバーしたい挙動:

1. トークン解決の優先順位 (`--token` > `$LP_TOKEN` (空白のみは無効) > ファイル)。
2. cwd 検出: `ProjectSettings/ProjectVersion.txt` を親方向に探索。
3. preferred port: 型違い / 範囲外は無視。
4. candidate_ports の構築: `--mode` 別の seed 順 / 隣接 5 個展開 / 重複除外 / `DEFAULT_PORTS` 後付け。
5. cache v2 ロード: 壊れたエントリ・型違いをサイレントスキップ。
6. discovery: alive 0/1/2+ × target/mode の組み合わせでメッセージとヒントが期待通り。
7. `exec`: `key=value` でないと fatal、`success=false` でも HTTP 200 なら exit 2。
8. `run` glob: `*` が `/` を跨ぐ、マッチ 0 件は fatal。
9. JUnit XML: pass / fail 両方の要素、`failedAtStep` 範囲外・`error` のみのフォールバック、特殊文字のエスケープ。
10. `project set-port` の `$schema` 追加 / 既存値保持。
11. `project unset-port` のパース失敗時に fatal (寛容にしない)。
12. `NO_COLOR` / non-TTY で色コードが出ない。

## 14. 現行 Python 実装の場所 (参考)

- `Tools~/liminal/liminal` — 1469 行のシングルファイル Python 3 スクリプト。
- `Tools~/liminal/README.md` — ユーザー向け README (本 SPEC と矛盾する場合は SPEC 優先で書き直してよい)。
- `Documentation~/ipc.md` — HTTP API の正本ドキュメント。
- `Documentation~/schemas/LiminalPalette.schema.json` — Project config の JSON Schema。
