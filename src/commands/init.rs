use std::path::{Path, PathBuf};

use anstream::println;
use anyhow::Result;
use serde_json::Value;

use crate::cache;
use crate::cli::InitArgs;
use crate::commands::project::{read_raw, require_project, write_config};
use crate::discovery::{Alive, Mode, candidate_ports, project_config_path, read_project_config};
use crate::http::{PROBE_TIMEOUT, probe_port};
use crate::style::{CYAN, DIM, GREEN, YELLOW};

/// `liminal init`。フラグ無しなら read-only で、副作用は一切起こさない (SPEC §4.2)。
pub(crate) fn run(args: &InitArgs, editor_port: Option<u16>, mode: Option<Mode>) -> Result<()> {
    if args.runtime_port == Some(0) {
        anyhow::bail!("--runtime-port は 1..=65535 の範囲で指定してください: 0");
    }
    let root = require_project()?;

    println!("{CYAN}Project{CYAN:#}");
    println!("  {}", root.display());
    println!();

    section_config(&root, editor_port, args.runtime_port)?;
    println!();
    section_token();
    println!();
    section_cli();
    println!();
    section_ai_skills(&root);
    println!();
    section_live_check(&root, mode);

    println!();
    println!("{GREEN}init complete{GREEN:#}");
    Ok(())
}

fn section_config(root: &Path, port: Option<u16>, runtime_port: Option<u16>) -> Result<()> {
    let path = project_config_path(root);
    println!("{CYAN}Project config{CYAN:#}");
    println!("  {}", path.display());

    let existing = read_raw(root);
    let writing = port.is_some() || runtime_port.is_some();

    // 書き込みが要求されたときだけファイルに触れる。
    if writing {
        let mut fields = existing.clone().unwrap_or(None).unwrap_or_default();
        let had_schema = fields.contains_key("$schema");
        if let Some(p) = port {
            fields.insert("port".into(), Value::from(p));
        }
        if let Some(p) = runtime_port {
            fields.insert("runtimePort".into(), Value::from(p));
        }
        write_config(root, &fields)?;

        if let Some(p) = port {
            println!("  {GREEN}set port = {p}{GREEN:#}");
        }
        if let Some(p) = runtime_port {
            println!("  {GREEN}set runtimePort = {p}{GREEN:#}");
        }
        if !had_schema {
            println!("  {DIM}$schema reference を追加しました{DIM:#}");
        }
    }

    let preferred = read_project_config(root);
    println!("  port        : {}", show_port(preferred.editor));
    println!("  runtimePort : {}", show_port(preferred.runtime));

    if !writing {
        match existing {
            Ok(None) => {
                println!("  {DIM}(no config file — `liminal init --port N` で作成できます){DIM:#}")
            }
            Ok(Some(fields)) if !fields.contains_key("$schema") => println!(
                "  {YELLOW}$schema がありません。`liminal init --port N` で再書き込みしてください{YELLOW:#}"
            ),
            Ok(Some(_)) => {}
            Err(msg) => println!("  {YELLOW}{msg}{YELLOW:#}"),
        }
    }
    Ok(())
}

fn section_token() {
    println!("{CYAN}Token{CYAN:#}");
    match token_path() {
        Some(p) => {
            let state = match std::fs::read_to_string(&p) {
                Ok(s) if s.trim().is_empty() => format!("{YELLOW}empty{YELLOW:#}"),
                Ok(_) => format!("{GREEN}exists{GREEN:#}"),
                Err(_) => format!("{YELLOW}missing{YELLOW:#}"),
            };
            println!("  {state}  {DIM}{}{DIM:#}", p.display());
        }
        None => println!("  {YELLOW}HOME が取得できません{YELLOW:#}"),
    }
}

fn section_cli() {
    println!("{CYAN}CLI{CYAN:#}");
    match std::env::current_exe() {
        Ok(p) => {
            println!("  {}", p.display());
            println!("  {DIM}ln -sf {} ~/.local/bin/liminal{DIM:#}", p.display());
        }
        Err(_) => println!("  {DIM}(バイナリパスを取得できません){DIM:#}"),
    }
}

fn section_ai_skills(root: &Path) {
    println!("{CYAN}AI Skills{CYAN:#}");
    let dir = root.join(".claude/skills");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("  {DIM}(no .claude/skills — {}){DIM:#}", dir.display());
        return;
    };

    let (mut installed, mut legacy) = (Vec::new(), Vec::new());
    for e in entries.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("liminal-") {
            installed.push(name);
        } else if name.starts_with("lp-") {
            legacy.push(name);
        }
    }
    installed.sort();
    legacy.sort();

    if installed.is_empty() {
        println!("  {DIM}(liminal-* skill が見つかりません){DIM:#}");
    } else {
        println!(
            "  {GREEN}{} skills{GREEN:#}  {DIM}{}{DIM:#}",
            installed.len(),
            installed.join(", ")
        );
    }
    if !legacy.is_empty() {
        println!(
            "  {YELLOW}legacy skill が残っています: {}{YELLOW:#}",
            legacy.join(", ")
        );
    }
}

fn section_live_check(root: &Path, mode: Option<Mode>) {
    let preferred = read_project_config(root);
    let cache = cache::load();
    let ports = candidate_ports(None, preferred, mode, &cache.entries());
    println!("{CYAN}Live check{CYAN:#}");

    let alive: Vec<Alive> = ports
        .iter()
        .filter_map(|p| probe_port(*p, PROBE_TIMEOUT).map(|b| Alive::from_health(*p, &b)))
        .collect();

    if alive.is_empty() {
        println!("  {DIM}(no live server — Unity を起動すると検出されます){DIM:#}");
        return;
    }
    for a in alive {
        println!(
            "  {GREEN}●{GREEN:#} {} [{}] {} {DIM}{}{DIM:#}",
            a.port, a.mode, a.project_name, a.project_path
        );
    }
}

fn token_path() -> Option<PathBuf> {
    let mut p = std::env::home_dir()?;
    p.push(".liminal-palette/token");
    Some(p)
}

fn show_port(p: Option<u16>) -> String {
    match p {
        Some(v) => v.to_string(),
        None => "(unset)".to_string(),
    }
}
