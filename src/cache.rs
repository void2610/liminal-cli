use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

/// キャッシュの形式バージョン。これ以外はサイレントに破棄する (SPEC §8)。
const CACHE_VERSION: u64 = 2;

const CACHE_FILE_NAME: &str = ".liminal-palette/ports.json";

/// `~/.liminal-palette/ports.json` の中身。
/// 読み書きともベストエフォートで、I/O も JSON エラーもすべて握り潰す。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct PortCache {
    /// key = 絶対 projectPath
    pub projects: BTreeMap<String, CachedProject>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct CachedProject {
    pub project_name: String,
    pub editor: Option<u16>,
    pub runtime: Option<u16>,
}

/// キャッシュ 1 件を「ポート 1 個」に展開したビュー。候補ポート組み立てと doctor の列挙で使う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheEntry {
    pub port: u16,
    pub mode: String,
    pub project_name: String,
    pub project_path: String,
}

impl PortCache {
    /// 全エントリを (projectPath, mode) 順で列挙する。BTreeMap なので順序は安定。
    pub(crate) fn entries(&self) -> Vec<CacheEntry> {
        let mut out = Vec::new();
        for (path, proj) in &self.projects {
            for (mode, port) in [("editor", proj.editor), ("runtime", proj.runtime)] {
                if let Some(p) = port {
                    out.push(CacheEntry {
                        port: p,
                        mode: mode.to_string(),
                        project_name: proj.project_name.clone(),
                        project_path: path.clone(),
                    });
                }
            }
        }
        out
    }

    /// 生存確認が取れたポートを記録する。同じ (projectPath, mode) は上書き。
    pub(crate) fn record(&mut self, project_path: &str, project_name: &str, mode: &str, port: u16) {
        // projectPath が空の応答はキーにできないので記録しない。
        if project_path.is_empty() {
            return;
        }
        let e = self.projects.entry(project_path.to_string()).or_default();
        e.project_name = project_name.to_string();
        // mode が editor / runtime のどちらでもない古いサーバは editor 扱い (SPEC §8)。
        if mode == "runtime" {
            e.runtime = Some(port);
        } else {
            e.editor = Some(port);
        }
    }

    /// 指定ポートのエントリを削除する。削除した件数を返す (`doctor --prune-stale` 用)。
    pub(crate) fn remove_port(&mut self, port: u16) -> usize {
        let mut removed = 0;
        for proj in self.projects.values_mut() {
            if proj.editor == Some(port) {
                proj.editor = None;
                removed += 1;
            }
            if proj.runtime == Some(port) {
                proj.runtime = None;
                removed += 1;
            }
        }
        // ポートが 1 つも残らなかったプロジェクトは捨てる。
        self.projects
            .retain(|_, p| p.editor.is_some() || p.runtime.is_some());
        removed
    }

    /// JSON 文字列へ。書き出しは indent=2 + 末尾改行 (SPEC §8)。
    pub(crate) fn to_json(&self) -> String {
        let mut projects = serde_json::Map::new();
        for (path, proj) in &self.projects {
            let mut ports = serde_json::Map::new();
            if let Some(p) = proj.editor {
                ports.insert("editor".into(), Value::from(p));
            }
            if let Some(p) = proj.runtime {
                ports.insert("runtime".into(), Value::from(p));
            }
            projects.insert(
                path.clone(),
                serde_json::json!({
                    "projectName": proj.project_name,
                    "ports": Value::Object(ports),
                }),
            );
        }
        let root = serde_json::json!({
            "version": CACHE_VERSION,
            "projects": Value::Object(projects),
        });
        format!(
            "{}\n",
            serde_json::to_string_pretty(&root).unwrap_or_default()
        )
    }

    /// JSON 文字列から。version 不一致・型違いのエントリはサイレントにスキップする。
    pub(crate) fn from_json(raw: &str) -> Self {
        let Ok(Value::Object(root)) = serde_json::from_str::<Value>(raw) else {
            return Self::default();
        };
        if root.get("version").and_then(Value::as_u64) != Some(CACHE_VERSION) {
            return Self::default();
        }
        let Some(Value::Object(projects)) = root.get("projects") else {
            return Self::default();
        };

        let mut out = Self::default();
        for (path, entry) in projects {
            let Some(obj) = entry.as_object() else {
                continue;
            };
            let ports = obj.get("ports").and_then(Value::as_object);
            let pick = |key: &str| -> Option<u16> {
                let n = ports?.get(key)?.as_u64()?;
                (1..=65535).contains(&n).then_some(n as u16)
            };
            let (editor, runtime) = (pick("editor"), pick("runtime"));
            // ポートが 1 つも読めなかったエントリは保持する意味がない。
            if editor.is_none() && runtime.is_none() {
                continue;
            }
            out.projects.insert(
                path.clone(),
                CachedProject {
                    project_name: obj
                        .get("projectName")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    editor,
                    runtime,
                },
            );
        }
        out
    }
}

/// キャッシュファイルのパス。HOME が取れない環境では None。
pub(crate) fn cache_path() -> Option<PathBuf> {
    let mut p = std::env::home_dir()?;
    p.push(CACHE_FILE_NAME);
    Some(p)
}

/// キャッシュを読む。読めなければ空を返す。
pub(crate) fn load() -> PortCache {
    cache_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|raw| PortCache::from_json(&raw))
        .unwrap_or_default()
}

/// キャッシュを書く。失敗は握り潰す (診断も表示もしない)。
pub(crate) fn save(cache: &PortCache) {
    let Some(path) = cache_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, cache.to_json());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PortCache {
        let mut c = PortCache::default();
        c.record("/dev/MyGame", "MyGame", "editor", 7610);
        c.record("/dev/MyGame", "MyGame", "runtime", 7611);
        c
    }

    #[test]
    fn cache_version2以外はサイレント破棄() {
        let raw = r#"{"version":1,"projects":{"/a":{"projectName":"A","ports":{"editor":7610}}}}"#;
        assert_eq!(PortCache::from_json(raw), PortCache::default());
    }

    #[test]
    fn cache_壊れたJSONは空() {
        assert_eq!(PortCache::from_json("{not json"), PortCache::default());
    }

    #[test]
    fn cache_往復できる() {
        let c = sample();
        assert_eq!(PortCache::from_json(&c.to_json()), c);
    }

    #[test]
    fn cache_型違いのportはスキップ() {
        let raw =
            r#"{"version":2,"projects":{"/a":{"projectName":"A","ports":{"editor":"7610"}}}}"#;
        assert_eq!(PortCache::from_json(raw), PortCache::default());
    }

    #[test]
    fn cache_範囲外のportはスキップ() {
        let raw = r#"{"version":2,"projects":{"/a":{"projectName":"A","ports":{"editor":0,"runtime":65536}}}}"#;
        assert_eq!(PortCache::from_json(raw), PortCache::default());
    }

    #[test]
    fn cache_片方だけ壊れていてももう片方は読める() {
        let raw = r#"{"version":2,"projects":{"/a":{"projectName":"A","ports":{"editor":7610,"runtime":"x"}}}}"#;
        let c = PortCache::from_json(raw);
        let p = &c.projects["/a"];
        assert_eq!((p.editor, p.runtime), (Some(7610), None));
    }

    #[test]
    fn cache_entriesはmode別に展開される() {
        let e = sample().entries();
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].port, e[0].mode.as_str()), (7610, "editor"));
        assert_eq!((e[1].port, e[1].mode.as_str()), (7611, "runtime"));
    }

    #[test]
    fn cache_未知modeはeditorとして記録される() {
        let mut c = PortCache::default();
        c.record("/a", "A", "", 7610);
        assert_eq!(c.projects["/a"].editor, Some(7610));
    }

    #[test]
    fn cache_projectPathが空なら記録しない() {
        let mut c = PortCache::default();
        c.record("", "A", "editor", 7610);
        assert!(c.projects.is_empty());
    }

    #[test]
    fn cache_record_は同じmodeを上書きする() {
        let mut c = sample();
        c.record("/dev/MyGame", "MyGame", "editor", 7620);
        assert_eq!(c.projects["/dev/MyGame"].editor, Some(7620));
    }

    #[test]
    fn cache_remove_port_で消えポートが無くなればプロジェクトごと消える() {
        let mut c = sample();
        assert_eq!(c.remove_port(7610), 1);
        assert_eq!(c.remove_port(7611), 1);
        assert!(c.projects.is_empty());
    }

    #[test]
    fn cache_remove_port_該当なしは0件() {
        let mut c = sample();
        assert_eq!(c.remove_port(9999), 0);
        assert_eq!(c.projects.len(), 1);
    }

    #[test]
    fn cache_書き出しは末尾改行付き() {
        assert!(sample().to_json().ends_with("}\n"));
    }
}
