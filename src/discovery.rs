use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::cache::CacheEntry;

/// Project config が宣言する preferred port (`port` / `runtimePort`)。
/// 不在・型違い・範囲外・壊れた JSON はいずれも該当フィールドを None にする (SPEC §9、致命にしない)。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreferredPorts {
    pub editor: Option<u16>,
    pub runtime: Option<u16>,
}

/// cwd から親方向に `ProjectSettings/ProjectVersion.txt` を探し、最初に見つけた dir を返す (SPEC §2)。
pub(crate) fn detect_project(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if dir.join("ProjectSettings/ProjectVersion.txt").is_file() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// `<project>/ProjectSettings/LiminalPalette.json` の preferred port を読む。
/// 読めない・壊れている場合も既定値を返すだけで、呼び出し側に失敗を伝えない。
pub(crate) fn read_project_config(project_root: &Path) -> PreferredPorts {
    let raw = match std::fs::read_to_string(project_config_path(project_root)) {
        Ok(s) => s,
        Err(_) => return PreferredPorts::default(),
    };
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&raw) else {
        return PreferredPorts::default();
    };
    PreferredPorts {
        editor: map.get("port").and_then(port_in_range),
        runtime: map.get("runtimePort").and_then(port_in_range),
    }
}

/// Project config のパス。
pub(crate) fn project_config_path(project_root: &Path) -> PathBuf {
    project_root.join("ProjectSettings/LiminalPalette.json")
}

// 整数かつ 1..=65535 のときだけ採用する。文字列や浮動小数、範囲外は None。
fn port_in_range(v: &Value) -> Option<u16> {
    let n = v.as_u64()?;
    if (1..=65535).contains(&n) {
        Some(n as u16)
    } else {
        None
    }
}

/// discovery の既定候補ポート (SPEC §2)。
pub(crate) const DEFAULT_PORTS: [u16; 6] = [7610, 7611, 7612, 7613, 7614, 7615];

/// 1 つの seed から広げる隣接ポート数 (seed 自身を含めて 6 個)。
const SEED_SPAN: u16 = 5;

/// `--mode` の指定。未指定は None。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Editor,
    Runtime,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Mode::Editor => "editor",
            Mode::Runtime => "runtime",
        }
    }
}

/// probe する候補ポート列を組み立てる (SPEC §5-3)。重複は最初に現れた位置を残して除外する。
pub(crate) fn candidate_ports(
    explicit_port: Option<u16>,
    preferred: PreferredPorts,
    mode: Option<Mode>,
    cache: &[CacheEntry],
) -> Vec<u16> {
    // --port 明示時は隣接探索もしない (SPEC §3)。
    if let Some(p) = explicit_port {
        return vec![p];
    }

    let mut out: Vec<u16> = Vec::new();
    let push = |port: u16, out: &mut Vec<u16>| {
        if port >= 1 && !out.contains(&port) {
            out.push(port);
        }
    };

    // seed の順序は mode によって変わる。
    let seeds: Vec<u16> = match mode {
        Some(Mode::Editor) => preferred.editor.into_iter().collect(),
        Some(Mode::Runtime) => [preferred.runtime, preferred.editor]
            .into_iter()
            .flatten()
            .collect(),
        None => [preferred.editor, preferred.runtime]
            .into_iter()
            .flatten()
            .collect(),
    };
    // 各 seed を seed..=seed+5 に展開する。u16 の上限を越える分は捨てる (65535 で overflow しない)。
    for seed in seeds {
        for offset in 0..=SEED_SPAN {
            match seed.checked_add(offset) {
                Some(p) => push(p, &mut out),
                None => break,
            }
        }
    }

    // キャッシュのうち mode が合うものを追加する。
    for e in cache {
        if mode.is_some_and(|m| m.as_str() != e.mode) {
            continue;
        }
        push(e.port, &mut out);
    }

    for p in DEFAULT_PORTS {
        push(p, &mut out);
    }
    out
}

/// `/health` の応答から discovery の判定に使う部分だけ取り出したもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Alive {
    pub port: u16,
    pub mode: String,
    pub project_name: String,
    pub project_path: String,
    pub version: String,
    pub command_count: u64,
}

impl Alive {
    /// `/health` の JSON object から組み立てる。欠けているフィールドは空文字 / 0 で埋める。
    pub(crate) fn from_health(port: u16, body: &serde_json::Map<String, Value>) -> Self {
        let s = |k: &str| {
            body.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Self {
            port,
            // mode が editor / runtime のどちらでもなければ editor とみなす (SPEC §5、古いサーバ互換)。
            mode: match body.get("mode").and_then(Value::as_str) {
                Some("runtime") => "runtime".to_string(),
                _ => "editor".to_string(),
            },
            project_name: s("projectName"),
            project_path: s("projectPath"),
            version: s("version"),
            command_count: body
                .get("commandCount")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        }
    }

    /// target (projectPath 完全一致 / projectName 完全一致 / パスとして正規化して一致) に合致するか。
    pub(crate) fn matches_project(&self, target: Option<&str>) -> bool {
        let Some(t) = target else { return true };
        if self.project_path == t || self.project_name == t {
            return true;
        }
        // target がパスとして実在するなら正規化して比較する。
        match std::fs::canonicalize(t) {
            Ok(canon) => canon.to_string_lossy() == self.project_path,
            Err(_) => false,
        }
    }

    pub(crate) fn matches_mode(&self, mode: Option<Mode>) -> bool {
        mode.is_none_or(|m| m.as_str() == self.mode)
    }
}

/// alive 一覧を target / mode で絞り込み、1 つに決める (SPEC §5-7)。
/// 決められない場合は利用者向けのヒント込みのエラーメッセージを返す。
pub(crate) fn select_alive(
    alive: Vec<Alive>,
    target: Option<&str>,
    mode: Option<Mode>,
) -> Result<Alive, String> {
    if alive.is_empty() {
        return Err("Liminal Palette サーバーが見つかりません".to_string());
    }

    let listing = format_alive_list(&alive);
    let filtered: Vec<Alive> = alive
        .iter()
        .filter(|a| a.matches_project(target) && a.matches_mode(mode))
        .cloned()
        .collect();

    if filtered.is_empty() {
        // target と mode のどちらで落ちたのかでメッセージを変える。
        if let Some(t) = target {
            return Err(format!(
                "指定のプロジェクト '{t}' に一致する Unity サーバーが見つかりません。\n生存中:\n{listing}"
            ));
        }
        if let Some(m) = mode {
            return Err(format!(
                "mode={} の Unity サーバーが生存していません。\n生存中:\n{listing}",
                m.as_str()
            ));
        }
        unreachable!("target も mode も未指定なら filtered は空にならない");
    }

    if filtered.len() == 1 {
        return Ok(filtered.into_iter().next().expect("len == 1"));
    }

    // 2 件以上。何を指定すれば絞れるかを状況から判断してヒントにする。
    let hint = disambiguation_hint(&filtered);
    if target.is_none() && mode.is_none() {
        return Err(format!(
            "複数の Unity プロジェクトが起動中です。{hint} で対象を指定してください。\n生存中:\n{listing}"
        ));
    }
    Err(format!(
        "対象を 1 つに絞れませんでした。{hint} で対象を指定してください。\n生存中:\n{}",
        format_alive_list(&filtered)
    ))
}

// 絞り込めなかった候補群を見て、どのオプションを足せば決まるかを返す (SPEC §5-7)。
fn disambiguation_hint(candidates: &[Alive]) -> &'static str {
    let first = &candidates[0].project_path;
    let same_path = candidates.iter().all(|a| &a.project_path == first);
    if same_path {
        // 同じプロジェクトで mode だけ違うなら mode で決まる。
        let modes_differ = candidates.iter().any(|a| a.mode != candidates[0].mode);
        if modes_differ {
            return "--mode editor|runtime";
        }
        // 同じ path で mode も同じなら、ポートを直接指定するしかない。
        return "--port";
    }
    "--project (または --mode)"
}

// エラーメッセージに載せる生存サーバ一覧。
fn format_alive_list(alive: &[Alive]) -> String {
    alive
        .iter()
        .map(|a| {
            format!(
                "  {} [{}] {} {}",
                a.port, a.mode, a.project_name, a.project_path
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// discovery の入力。CLI のグローバルオプションと環境変数をまとめたもの。
#[derive(Debug, Default, Clone)]
pub(crate) struct DiscoveryOptions {
    pub base_url: Option<String>,
    pub port: Option<u16>,
    pub project: Option<String>,
    pub mode: Option<Mode>,
}

/// discovery の結果。`/health` の body は呼び出し側 (health コマンド) が再利用する。
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub base_url: String,
    pub health: Option<Map<String, Value>>,
}

/// ターゲットプロジェクトを解決する (SPEC §2)。
/// `--project` の値がディレクトリとして実在すれば絶対パスに正規化し、そうでなければ名前として扱う。
pub(crate) fn resolve_target(project: Option<&str>) -> Option<String> {
    let raw = match project {
        Some(p) => p.to_string(),
        None => match std::env::var("LP_PROJECT").ok().filter(|s| !s.is_empty()) {
            Some(v) => v,
            // 明示指定も環境変数も無ければ cwd から検出したプロジェクトを既定のターゲットにする。
            // これが無いと、プロジェクト内で作業していても別プロジェクトのサーバが選ばれてしまう。
            None => {
                let root = std::env::current_dir()
                    .ok()
                    .and_then(|c| detect_project(&c))?;
                return Some(root.to_string_lossy().to_string());
            }
        },
    };
    match std::fs::canonicalize(&raw) {
        Ok(p) if p.is_dir() => Some(p.to_string_lossy().to_string()),
        _ => Some(raw),
    }
}

/// 接続先を決める (SPEC §5)。
pub(crate) fn resolve(opts: &DiscoveryOptions) -> anyhow::Result<Resolved> {
    // --base-url 明示時は discovery をバイパスする。probe は best-effort で、失敗しても致命にしない。
    if let Some(url) = &opts.base_url {
        return Ok(Resolved {
            base_url: url.clone(),
            health: crate::http::probe_url(url, crate::http::BASE_URL_PROBE_TIMEOUT),
        });
    }

    let target = resolve_target(opts.project.as_deref());
    // preferred port はターゲットのディレクトリから読む。--project で外から指定された場合に
    // cwd の設定を見てしまうと、そのプロジェクトの preferred port を probe できない。
    let preferred = target
        .as_deref()
        .map(Path::new)
        .filter(|p| p.is_dir())
        .map(read_project_config)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| detect_project(&cwd))
                .map(|root| read_project_config(&root))
        })
        .unwrap_or_default();

    let mut cache = crate::cache::load();
    let cache_entries = cache.entries();

    // キャッシュに target 一致のポートがあれば先に試す (SPEC §5-4)。当たればそこで確定。
    // --port 明示時はこの早出しを行わない (指定を無視して別ポートを選んでしまうため)。
    if let Some(t) = target.as_deref()
        && opts.port.is_none()
    {
        for e in cache_entries.iter().filter(|e| {
            opts.mode.is_none_or(|m| m.as_str() == e.mode)
                // probe 前にキャッシュ上の名前 / パスで絞る。無関係なプロジェクトを
                // 1 件ごとに 400ms 待って試すと、早出しの意味が無くなる。
                && (e.project_path == t || e.project_name == t)
        }) {
            let Some(body) = crate::http::probe_port(e.port, crate::http::PROBE_TIMEOUT) else {
                continue;
            };
            let a = Alive::from_health(e.port, &body);
            if a.matches_project(Some(t)) && a.matches_mode(opts.mode) {
                record_and_save(&mut cache, &a);
                return Ok(Resolved {
                    base_url: base_url_for(a.port),
                    health: Some(body),
                });
            }
        }
    }

    // 候補ポートを総当たりで probe する。表示順を保つため候補順のまま集める。
    let candidates = candidate_ports(opts.port, preferred, opts.mode, &cache_entries);
    let mut alive: Vec<Alive> = Vec::new();
    let mut bodies: Vec<(u16, Map<String, Value>)> = Vec::new();
    for (port, probed) in candidates.iter().zip(crate::http::probe_all(
        &candidates,
        crate::http::PROBE_TIMEOUT,
    )) {
        if let Some(body) = probed {
            alive.push(Alive::from_health(*port, &body));
            bodies.push((*port, body));
        }
    }

    if alive.is_empty() {
        let ports = candidates
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("Liminal Palette サーバーが見つかりません (試したポート: {ports})");
    }

    let selected = select_alive(alive, target.as_deref(), opts.mode).map_err(anyhow::Error::msg)?;
    record_and_save(&mut cache, &selected);
    let health = bodies
        .into_iter()
        .find(|(p, _)| *p == selected.port)
        .map(|(_, b)| b);
    Ok(Resolved {
        base_url: base_url_for(selected.port),
        health,
    })
}

pub(crate) fn base_url_for(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

// 採用したポートをキャッシュに書き戻す。保存失敗は握り潰す (SPEC §8)。
fn record_and_save(cache: &mut crate::cache::PortCache, a: &Alive) {
    cache.record(&a.project_path, &a.project_name, &a.mode, a.port);
    crate::cache::save(cache);
}

#[cfg(test)]
// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // <root>/ProjectSettings/ProjectVersion.txt を作って root を返す
    fn make_unity_project(root: &Path) {
        let ps = root.join("ProjectSettings");
        fs::create_dir_all(&ps).unwrap();
        fs::write(
            ps.join("ProjectVersion.txt"),
            "m_EditorVersion: 6000.0.0f1\n",
        )
        .unwrap();
    }

    #[test]
    fn cwd直下にマーカーがあれば_cwd_を返す() {
        let tmp = TempDir::new().unwrap();
        make_unity_project(tmp.path());

        let found = detect_project(tmp.path());
        assert_eq!(found.as_deref(), Some(tmp.path()));
    }

    #[test]
    fn 親階層にマーカーがあれば_その親を返す() {
        let tmp = TempDir::new().unwrap();
        make_unity_project(tmp.path());
        let deep = tmp.path().join("Assets/Scripts/Foo");
        fs::create_dir_all(&deep).unwrap();

        let found = detect_project(&deep);
        assert_eq!(found.as_deref(), Some(tmp.path()));
    }

    #[test]
    fn rootまでマーカーが無ければ_None() {
        let tmp = TempDir::new().unwrap();
        let deep = tmp.path().join("a/b/c");
        fs::create_dir_all(&deep).unwrap();

        assert_eq!(detect_project(&deep), None);
    }

    #[test]
    fn 存在しないパスでも_panicせず_None() {
        let tmp = TempDir::new().unwrap();
        let gone = tmp.path().join("does/not/exist");

        assert_eq!(detect_project(&gone), None);
    }

    // ---- candidate_ports (SPEC §5-3) ----

    fn pref(editor: Option<u16>, runtime: Option<u16>) -> PreferredPorts {
        PreferredPorts { editor, runtime }
    }

    fn cache_entry(port: u16, mode: &str) -> CacheEntry {
        CacheEntry {
            port,
            mode: mode.to_string(),
            project_name: "N".into(),
            project_path: "/p".into(),
        }
    }

    #[test]
    fn 候補_preferredもcacheも無ければ_DEFAULTのみ() {
        let got = candidate_ports(None, PreferredPorts::default(), None, &[]);
        assert_eq!(got, DEFAULT_PORTS.to_vec());
    }

    #[test]
    fn 候補_preferred_editor_は隣接5個に展開され重複除外される() {
        let got = candidate_ports(None, pref(Some(7613), None), None, &[]);
        // 7613..=7618 の 6 個 + DEFAULT のうち未出の 7610,7611,7612 = 9 個
        assert_eq!(
            got,
            vec![7613, 7614, 7615, 7616, 7617, 7618, 7610, 7611, 7612]
        );
    }

    #[test]
    fn 候補_上限付近でも_overflow_しない() {
        let got = candidate_ports(None, pref(Some(65535), None), None, &[]);
        let mut want = vec![65535];
        want.extend(DEFAULT_PORTS);
        assert_eq!(got, want);
    }

    #[test]
    fn 候補_explicit_port_は1個だけ() {
        let got = candidate_ports(
            Some(9000),
            pref(Some(7613), Some(7700)),
            None,
            &[cache_entry(7611, "editor")],
        );
        assert_eq!(got, vec![9000]);
    }

    #[test]
    fn 候補_mode未指定は_editor_runtime_の順に_seed() {
        let got = candidate_ports(None, pref(Some(7620), Some(7630)), None, &[]);
        assert_eq!(&got[..6], &[7620, 7621, 7622, 7623, 7624, 7625]);
        assert_eq!(&got[6..12], &[7630, 7631, 7632, 7633, 7634, 7635]);
    }

    #[test]
    fn 候補_mode_runtime_は_runtime_を先に_seed() {
        let got = candidate_ports(None, pref(Some(7620), Some(7630)), Some(Mode::Runtime), &[]);
        assert_eq!(&got[..6], &[7630, 7631, 7632, 7633, 7634, 7635]);
        assert_eq!(&got[6..12], &[7620, 7621, 7622, 7623, 7624, 7625]);
    }

    #[test]
    fn 候補_mode_editor_は_runtime_を_seed_にしない() {
        let got = candidate_ports(None, pref(Some(7620), Some(7630)), Some(Mode::Editor), &[]);
        assert!(
            !got.contains(&7631),
            "runtime seed の隣接が混ざっている: {got:?}"
        );
        assert_eq!(&got[..6], &[7620, 7621, 7622, 7623, 7624, 7625]);
    }

    #[test]
    fn 候補_cacheはDEFAULTより先に入る() {
        let got = candidate_ports(
            None,
            PreferredPorts::default(),
            None,
            &[cache_entry(7700, "editor")],
        );
        assert_eq!(got[0], 7700);
    }

    #[test]
    fn 候補_mode指定時はcacheもmodeで絞る() {
        let cache = [cache_entry(7700, "editor"), cache_entry(7800, "runtime")];
        let got = candidate_ports(None, PreferredPorts::default(), Some(Mode::Runtime), &cache);
        assert!(got.contains(&7800));
        assert!(!got.contains(&7700));
    }

    // ---- Alive (SPEC §5 のマッチャ) ----

    fn alive(mode: &str, name: &str, path: &str) -> Alive {
        let body = serde_json::json!({
            "mode": mode, "projectName": name, "projectPath": path,
            "version": "0.2.0", "commandCount": 3,
        });
        Alive::from_health(7610, body.as_object().unwrap())
    }

    #[test]
    fn alive_未知modeはeditor扱い() {
        assert_eq!(alive("", "A", "/a").mode, "editor");
        assert_eq!(alive("weird", "A", "/a").mode, "editor");
        assert_eq!(alive("runtime", "A", "/a").mode, "runtime");
    }

    #[test]
    fn alive_欠損フィールドは空で埋まる() {
        let body = serde_json::json!({});
        let a = Alive::from_health(7610, body.as_object().unwrap());
        assert_eq!((a.project_name.as_str(), a.command_count), ("", 0));
    }

    #[test]
    fn alive_target未指定は常に一致() {
        assert!(alive("editor", "A", "/a").matches_project(None));
    }

    #[test]
    fn alive_projectName一致とprojectPath一致() {
        let a = alive("editor", "MyGame", "/dev/MyGame");
        assert!(a.matches_project(Some("MyGame")));
        assert!(a.matches_project(Some("/dev/MyGame")));
        assert!(!a.matches_project(Some("Other")));
    }

    #[test]
    fn alive_mode一致() {
        let a = alive("runtime", "A", "/a");
        assert!(a.matches_mode(None));
        assert!(a.matches_mode(Some(Mode::Runtime)));
        assert!(!a.matches_mode(Some(Mode::Editor)));
    }

    // ---- select_alive (SPEC §5-7) ----

    fn al(port: u16, mode: &str, name: &str, path: &str) -> Alive {
        let body = serde_json::json!({
            "mode": mode, "projectName": name, "projectPath": path,
            "version": "0.2.0", "commandCount": 1,
        });
        Alive::from_health(port, body.as_object().unwrap())
    }

    #[test]
    fn 選択_alive0件はエラー() {
        let e = select_alive(vec![], None, None).unwrap_err();
        assert!(e.contains("見つかりません"), "{e}");
    }

    #[test]
    fn 選択_alive1件はそのまま採用() {
        let a = al(7610, "editor", "A", "/a");
        assert_eq!(select_alive(vec![a.clone()], None, None).unwrap(), a);
    }

    #[test]
    fn 選択_指定なしで2件以上は曖昧エラー() {
        let e = select_alive(
            vec![al(7610, "editor", "A", "/a"), al(7611, "editor", "B", "/b")],
            None,
            None,
        )
        .unwrap_err();
        assert!(e.contains("複数の Unity プロジェクトが起動中です"), "{e}");
        // 生存中の一覧が添えられる
        assert!(e.contains("7610 [editor] A /a"), "{e}");
        assert!(e.contains("7611 [editor] B /b"), "{e}");
    }

    #[test]
    fn 選択_projectで絞れる() {
        let got = select_alive(
            vec![al(7610, "editor", "A", "/a"), al(7611, "editor", "B", "/b")],
            Some("B"),
            None,
        )
        .unwrap();
        assert_eq!(got.port, 7611);
    }

    #[test]
    fn 選択_modeで絞れる() {
        let got = select_alive(
            vec![
                al(7610, "editor", "A", "/a"),
                al(7611, "runtime", "A", "/a"),
            ],
            None,
            Some(Mode::Runtime),
        )
        .unwrap();
        assert_eq!(got.port, 7611);
    }

    #[test]
    fn 選択_target不一致はプロジェクト名入りのエラー() {
        let e = select_alive(vec![al(7610, "editor", "A", "/a")], Some("Nope"), None).unwrap_err();
        assert!(e.contains("指定のプロジェクト 'Nope'"), "{e}");
        assert!(e.contains("生存中:"), "{e}");
    }

    #[test]
    fn 選択_mode不一致はmode入りのエラー() {
        let e = select_alive(
            vec![al(7610, "editor", "A", "/a")],
            None,
            Some(Mode::Runtime),
        )
        .unwrap_err();
        assert!(e.contains("mode=runtime"), "{e}");
    }

    #[test]
    fn 選択_同一プロジェクトでmode違いなら_modeヒント() {
        let e = select_alive(
            vec![
                al(7610, "editor", "A", "/a"),
                al(7611, "runtime", "A", "/a"),
            ],
            None,
            None,
        )
        .unwrap_err();
        assert!(e.contains("--mode editor|runtime"), "{e}");
    }

    #[test]
    fn 選択_projectPath違いなら_projectヒント() {
        let e = select_alive(
            vec![al(7610, "editor", "A", "/a"), al(7611, "editor", "B", "/b")],
            None,
            None,
        )
        .unwrap_err();
        assert!(e.contains("--project"), "{e}");
    }

    #[test]
    fn 選択_同一プロジェクト同一modeなら_portヒント() {
        let e = select_alive(
            vec![al(7610, "editor", "A", "/a"), al(7611, "editor", "A", "/a")],
            None,
            None,
        )
        .unwrap_err();
        assert!(e.contains("--port"), "{e}");
    }

    #[test]
    fn 選択_フィルタ後も2件以上なら絞れないエラー() {
        let e = select_alive(
            vec![al(7610, "editor", "A", "/a"), al(7611, "editor", "A", "/a")],
            Some("A"),
            None,
        )
        .unwrap_err();
        assert!(e.contains("絞れませんでした"), "{e}");
    }

    // <root>/ProjectSettings/LiminalPalette.json に raw を書く
    fn write_config(root: &Path, raw: &str) {
        let ps = root.join("ProjectSettings");
        fs::create_dir_all(&ps).unwrap();
        fs::write(ps.join("LiminalPalette.json"), raw).unwrap();
    }

    #[test]
    fn config_ファイル不在なら_両方None() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(read_project_config(tmp.path()), PreferredPorts::default());
    }

    #[test]
    fn config_port_と_runtimePort_を読む() {
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), r#"{"port":7613,"runtimePort":7700}"#);

        let got = read_project_config(tmp.path());
        assert_eq!(
            got,
            PreferredPorts {
                editor: Some(7613),
                runtime: Some(7700),
            }
        );
    }

    #[test]
    fn config_型違いの_port_は_None() {
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), r#"{"port":"7613"}"#);

        assert_eq!(read_project_config(tmp.path()), PreferredPorts::default());
    }

    #[test]
    fn config_範囲外の_port_は_None() {
        let tmp = TempDir::new().unwrap();

        write_config(tmp.path(), r#"{"port":0}"#);
        assert_eq!(read_project_config(tmp.path()).editor, None);

        write_config(tmp.path(), r#"{"port":65536}"#);
        assert_eq!(read_project_config(tmp.path()).editor, None);
    }

    #[test]
    fn config_壊れた_JSON_は_両方None() {
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), r#"{"port":7613"#);

        assert_eq!(read_project_config(tmp.path()), PreferredPorts::default());
    }

    #[test]
    fn config_未知フィールド共存でも_既知は読める() {
        let tmp = TempDir::new().unwrap();
        write_config(
            tmp.path(),
            r#"{"$schema":"https://example.com/s.json","port":7613,"extra":true}"#,
        );

        let got = read_project_config(tmp.path());
        assert_eq!(got.editor, Some(7613));
        assert_eq!(got.runtime, None);
    }
}
