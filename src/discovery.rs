use std::path::{Path, PathBuf};

use serde_json::Value;

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
    let mut push = |port: u16, out: &mut Vec<u16>| {
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

#[cfg(test)]
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
