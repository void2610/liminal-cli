use std::path::{Path, PathBuf};

use anstream::println;
use anyhow::{Result, bail};
use serde_json::{Map, Value};

use crate::cache;
use crate::cli::{ProjectCommand, SetPortArgs, UnsetPortArgs};
use crate::discovery::{
    Alive, Mode, PreferredPorts, candidate_ports, detect_project, project_config_path,
    read_project_config,
};
use crate::http::{PROBE_TIMEOUT, probe_port};
use crate::style::{CYAN, DIM, GREEN, YELLOW};

/// Project config に自動付与する JSON Schema の URL (SPEC §2)。
const CANONICAL_SCHEMA: &str = "https://raw.githubusercontent.com/void2610/liminal-palette/main/Documentation~/schemas/LiminalPalette.schema.json";

pub(crate) fn run(sub: &ProjectCommand, mode: Option<Mode>) -> Result<()> {
    let root = require_project()?;
    match sub {
        ProjectCommand::Show => show(&root, mode),
        ProjectCommand::SetPort(args) => set_port(&root, args),
        ProjectCommand::UnsetPort(args) => unset_port(&root, args),
    }
}

/// cwd から Unity プロジェクトを必須で解決する。見つからなければ致命エラー (SPEC §4.4)。
pub(crate) fn require_project() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    match detect_project(&cwd) {
        Some(p) => Ok(p),
        None => bail!(
            "Unity プロジェクトが見つかりません (cwd から親方向に ProjectSettings/ProjectVersion.txt を探しました): {}",
            cwd.display()
        ),
    }
}

/// 設定ファイルの JSON object を読む。
/// `Ok(None)` はファイル不在、`Err(_)` はパース失敗 (呼び出し側が寛容にするか決める)。
pub(crate) fn read_raw(root: &Path) -> Result<Option<Map<String, Value>>, String> {
    let path = project_config_path(root);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(m)) => Ok(Some(m)),
        _ => Err(format!("{} の JSON を解釈できません", path.display())),
    }
}

/// 設定ファイルを書き出す。`$schema` を必ず先頭に置き、末尾に改行 1 個を付ける (SPEC §9)。
pub(crate) fn write_config(root: &Path, fields: &Map<String, Value>) -> Result<()> {
    let path = project_config_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    let mut out = Map::new();
    let schema = fields
        .get("$schema")
        .cloned()
        .unwrap_or_else(|| Value::from(CANONICAL_SCHEMA));
    out.insert("$schema".into(), schema);
    for (k, v) in fields {
        if k != "$schema" {
            out.insert(k.clone(), v.clone());
        }
    }

    let body = serde_json::to_string_pretty(&Value::Object(out))?;
    std::fs::write(&path, format!("{body}\n"))?;
    Ok(())
}

/// 対象フィールド名。`--runtime` なら runtimePort。
pub(crate) fn field_name(runtime: bool) -> &'static str {
    if runtime { "runtimePort" } else { "port" }
}

fn set_port(root: &Path, args: &SetPortArgs) -> Result<()> {
    if !(1..=65535).contains(&args.port) {
        bail!("PORT は 1..=65535 の範囲で指定してください: {}", args.port);
    }

    // set-port は壊れた JSON に寛容。警告だけ出して上書きする (SPEC §4.4)。
    let mut fields = match read_raw(root) {
        Ok(existing) => existing.unwrap_or_default(),
        Err(msg) => {
            println!("{YELLOW}warn{YELLOW:#}  {msg} — 上書きします");
            Map::new()
        }
    };

    let key = field_name(args.runtime);
    let had_schema = fields.contains_key("$schema");
    fields.insert(key.to_string(), Value::from(args.port));
    write_config(root, &fields)?;

    let path = project_config_path(root);
    println!("{GREEN}set{GREEN:#}  {key} = {}", args.port);
    println!("  {DIM}{}{DIM:#}", path.display());
    if !had_schema {
        println!("  {DIM}$schema reference を追加しました{DIM:#}");
    }
    Ok(())
}

fn unset_port(root: &Path, args: &UnsetPortArgs) -> Result<()> {
    let path = project_config_path(root);
    // unset-port はパース失敗を致命扱いにする。上書きすると利用者の意図を壊すため (SPEC §4.4)。
    let existing = read_raw(root).map_err(anyhow::Error::msg)?;
    let Some(mut fields) = existing else {
        println!("  {DIM}(no config file — {}){DIM:#}", path.display());
        return Ok(());
    };

    let key = field_name(args.runtime);
    if fields.remove(key).is_none() {
        println!("  {DIM}{key} は設定されていません{DIM:#}");
        return Ok(());
    }

    // $schema 以外のユーザーキーが残らないならファイルごと消す (SPEC §9)。
    let user_keys = fields.keys().filter(|k| *k != "$schema").count();
    if user_keys == 0 {
        std::fs::remove_file(&path)?;
        println!("{GREEN}removed{GREEN:#}  {}", path.display());
        return Ok(());
    }

    write_config(root, &fields)?;
    println!("{GREEN}updated{GREEN:#}  {key} を削除しました");
    println!("  {DIM}{}{DIM:#}", path.display());
    Ok(())
}

fn show(root: &Path, mode: Option<Mode>) -> Result<()> {
    let path = project_config_path(root);
    let preferred = read_project_config(root);

    println!("{CYAN}Project{CYAN:#}");
    println!("  {}", root.display());
    println!();
    println!("{CYAN}Project config{CYAN:#}");
    println!("  {}", path.display());
    println!("  port        : {}", show_port(preferred.editor));
    println!("  runtimePort : {}", show_port(preferred.runtime));

    match std::fs::read_to_string(&path) {
        Ok(raw) => {
            for line in raw.lines() {
                println!("  {DIM}{line}{DIM:#}");
            }
        }
        Err(_) => println!("  {DIM}(no config file){DIM:#}"),
    }

    println!();
    println!("{CYAN}Live listeners{CYAN:#}");
    let listeners = probe_project_listeners(root, preferred, mode);
    if listeners.is_empty() {
        println!("  {DIM}(no live server for this project){DIM:#}");
    }
    for a in listeners {
        println!(
            "  {GREEN}●{GREEN:#} {} [{}] {} {DIM}{}{DIM:#}",
            a.port,
            a.mode,
            a.project_name,
            port_tag(a.port, preferred)
        );
    }
    Ok(())
}

// このプロジェクト宛の live listener を候補ポート順で集める。
fn probe_project_listeners(
    root: &Path,
    preferred: PreferredPorts,
    mode: Option<Mode>,
) -> Vec<Alive> {
    let cache = cache::load();
    let root_str = root.to_string_lossy().to_string();
    candidate_ports(None, preferred, mode, &cache.entries())
        .into_iter()
        .filter_map(|p| probe_port(p, PROBE_TIMEOUT).map(|b| Alive::from_health(p, &b)))
        .filter(|a| a.project_path == root_str)
        .collect()
}

// preferred と一致したポートにタグを付ける (SPEC §4.4)。
fn port_tag(port: u16, preferred: PreferredPorts) -> String {
    match (preferred.editor, preferred.runtime) {
        (Some(e), None) if e == port => "(matches port, runtimePort unset)".into(),
        (Some(e), _) if e == port => "(matches port)".into(),
        (_, Some(r)) if r == port => "(matches runtimePort)".into(),
        _ => String::new(),
    }
}

fn show_port(p: Option<u16>) -> String {
    match p {
        Some(v) => v.to_string(),
        None => "(unset)".to_string(),
    }
}

#[cfg(test)]
// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn read_back(root: &Path) -> String {
        std::fs::read_to_string(project_config_path(root)).unwrap()
    }

    #[test]
    fn 設定書き出しは_schema_が先頭で末尾に改行() {
        let tmp = TempDir::new().unwrap();
        let mut f = Map::new();
        f.insert("port".into(), Value::from(7613));
        write_config(tmp.path(), &f).unwrap();

        let raw = read_back(tmp.path());
        assert!(raw.ends_with("}\n"));
        let first_key = raw.lines().nth(1).unwrap();
        assert!(first_key.contains("$schema"), "{raw}");
        assert!(first_key.contains(CANONICAL_SCHEMA), "{raw}");
    }

    #[test]
    fn 設定書き出しは既存_schema_を保持する() {
        let tmp = TempDir::new().unwrap();
        let mut f = Map::new();
        f.insert(
            "$schema".into(),
            Value::from("https://example.com/custom.json"),
        );
        f.insert("port".into(), Value::from(7613));
        write_config(tmp.path(), &f).unwrap();

        assert!(read_back(tmp.path()).contains("https://example.com/custom.json"));
    }

    #[test]
    fn 設定書き出しは未知フィールドを残す() {
        let tmp = TempDir::new().unwrap();
        let mut f = Map::new();
        f.insert("extra".into(), Value::from(true));
        f.insert("port".into(), Value::from(7613));
        write_config(tmp.path(), &f).unwrap();

        let raw = read_back(tmp.path());
        assert!(raw.contains("\"extra\""), "{raw}");
        assert!(raw.contains("\"port\""), "{raw}");
    }

    #[test]
    fn read_raw_はファイル不在で_None() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(read_raw(tmp.path()), Ok(None));
    }

    #[test]
    fn read_raw_は壊れたJSONで_Err() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("ProjectSettings")).unwrap();
        std::fs::write(project_config_path(tmp.path()), "{broken").unwrap();
        assert!(read_raw(tmp.path()).is_err());
    }

    #[test]
    fn field_name_は_runtime_で切り替わる() {
        assert_eq!(field_name(false), "port");
        assert_eq!(field_name(true), "runtimePort");
    }

    #[test]
    fn port_tag_は_preferred_と一致したときだけ付く() {
        let p = PreferredPorts {
            editor: Some(7613),
            runtime: None,
        };
        assert_eq!(port_tag(7613, p), "(matches port, runtimePort unset)");
        assert_eq!(port_tag(7700, p), "");

        let both = PreferredPorts {
            editor: Some(7613),
            runtime: Some(7700),
        };
        assert_eq!(port_tag(7613, both), "(matches port)");
        assert_eq!(port_tag(7700, both), "(matches runtimePort)");
    }
}
