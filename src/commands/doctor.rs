use anstream::println;
use anyhow::Result;

use crate::cache;
use crate::cli::DoctorArgs;
use crate::discovery::{
    Alive, Mode, candidate_ports, detect_project, read_project_config, resolve_target, select_alive,
};
use crate::http::{PROBE_TIMEOUT, probe_all};
use crate::style::{CYAN, DIM, GREEN, YELLOW};

/// `liminal doctor`。純粋な診断なので、どんな状態でも Ok(()) で返す (SPEC §4.3)。
pub(crate) fn run(
    args: &DoctorArgs,
    project: Option<&str>,
    mode: Option<Mode>,
    port: Option<u16>,
) -> Result<()> {
    section_token();
    println!();
    let preferred = section_project(project, mode);
    println!();

    let mut cache = cache::load();
    section_cache(&cache);
    println!();

    // 候補ポートに加えて、キャッシュ上の全ポートも 1 回ずつ probe する。
    let entries = cache.entries();
    let mut ports = candidate_ports(port, preferred, mode, &entries);
    for e in &entries {
        if !ports.contains(&e.port) {
            ports.push(e.port);
        }
    }

    println!("{CYAN}Live probe ({} ports){CYAN:#}", ports.len());
    let mut alive: Vec<Alive> = Vec::new();
    let mut dead: Vec<u16> = Vec::new();
    for (p, probed) in ports.iter().zip(probe_all(&ports, PROBE_TIMEOUT)) {
        match probed {
            Some(body) => {
                let a = Alive::from_health(*p, &body);
                println!(
                    "  {GREEN}●{GREEN:#} {} [{}] {} v{} cmds={} {DIM}{}{DIM:#}",
                    a.port, a.mode, a.project_name, a.version, a.command_count, a.project_path
                );
                alive.push(a);
            }
            None => {
                println!("  {DIM}○ {p} (no response){DIM:#}");
                dead.push(*p);
            }
        }
    }

    println!();
    section_resolution(alive, project, mode);

    if args.prune_stale {
        let removed: usize = dead.iter().map(|p| cache.remove_port(*p)).sum();
        cache::save(&cache);
        println!();
        let unit = if removed == 1 { "entry" } else { "entries" };
        println!("  {DIM}pruned {removed} stale cache {unit}{DIM:#}");
    }
    Ok(())
}

fn section_token() {
    println!("{CYAN}Token{CYAN:#}");
    let env = std::env::var("LP_TOKEN").ok();
    match env.as_deref() {
        Some(v) if v.trim().is_empty() => println!("  $LP_TOKEN      : {YELLOW}empty{YELLOW:#}"),
        Some(_) => println!("  $LP_TOKEN      : {GREEN}set{GREEN:#}"),
        None => println!("  $LP_TOKEN      : {DIM}unset{DIM:#}"),
    }

    let path = std::env::home_dir().map(|mut p| {
        p.push(".liminal-palette/token");
        p
    });
    match &path {
        Some(p) => {
            let state = match std::fs::read_to_string(p) {
                Ok(s) if s.trim().is_empty() => format!("{YELLOW}empty{YELLOW:#}"),
                Ok(_) => format!("{GREEN}exists{GREEN:#}"),
                Err(_) => format!("{DIM}missing{DIM:#}"),
            };
            println!("  token file     : {state}  {DIM}{}{DIM:#}", p.display());
        }
        None => println!("  token file     : {YELLOW}HOME 不明{YELLOW:#}"),
    }
}

fn section_project(project: Option<&str>, mode: Option<Mode>) -> crate::discovery::PreferredPorts {
    println!("{CYAN}Project detection{CYAN:#}");
    let cwd = std::env::current_dir().ok();
    println!(
        "  cwd            : {}",
        cwd.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(unavailable)".into())
    );

    let detected = cwd.as_ref().and_then(|c| detect_project(c));
    println!(
        "  detected       : {}",
        detected
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(none)".into())
    );
    println!("  --project      : {}", opt(project));
    println!(
        "  $LP_PROJECT    : {}",
        opt(std::env::var("LP_PROJECT").ok().as_deref())
    );
    println!(
        "  resolved target: {}",
        opt(resolve_target(project).as_deref())
    );
    println!("  --mode         : {}", opt(mode.map(Mode::as_str)));

    let preferred = detected
        .as_ref()
        .map(|p| read_project_config(p))
        .unwrap_or_default();
    println!(
        "  preferred      : port={} runtimePort={}",
        preferred
            .editor
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(unset)".into()),
        preferred
            .runtime
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(unset)".into())
    );
    preferred
}

fn section_cache(cache: &cache::PortCache) {
    println!("{CYAN}Port cache{CYAN:#}");
    let entries = cache.entries();
    if entries.is_empty() {
        println!("  {DIM}(empty){DIM:#}");
        return;
    }
    for e in entries {
        println!(
            "  port {} [{}] {} {DIM}{}{DIM:#}",
            e.port, e.mode, e.project_name, e.project_path
        );
    }
}

fn section_resolution(alive: Vec<Alive>, project: Option<&str>, mode: Option<Mode>) {
    println!("{CYAN}Resolution{CYAN:#}");
    let target = resolve_target(project);
    match select_alive(alive, target.as_deref(), mode) {
        Ok(a) => println!(
            "  {GREEN}selected{GREEN:#}  {} [{}] {} {DIM}{}{DIM:#}",
            a.port, a.mode, a.project_name, a.project_path
        ),
        // doctor は失敗しても exit 0。メッセージだけ見せる。
        Err(msg) => {
            for line in msg.lines() {
                println!("  {YELLOW}{line}{YELLOW:#}");
            }
        }
    }
}

fn opt(v: Option<&str>) -> String {
    match v {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => "(none)".to_string(),
    }
}
