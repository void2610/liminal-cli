use crate::error::ExecFailure;
use crate::http::{CommandsResponse, ExecResponse, HealthResponse};
use crate::style::{BOLD, CYAN, DIM, GREEN, RED, YELLOW};
use anstream::println;
use anyhow::Result;

pub(crate) fn render_health(body: &HealthResponse, url: &str, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(body)?);
        return Ok(());
    }

    println!("{GREEN}ok{GREEN:#}  {url}");
    println!("  version       : {}", body.version);
    println!("  mode          : {}", show_or_unknown(&body.mode));
    println!("  projectName   : {}", show_or_unknown(&body.project_name));
    println!("  projectPath   : {}", show_or_unknown(&body.project_path));
    println!("  commandCount  : {}", body.command_count);
    Ok(())
}

pub(crate) fn render_commands(body: &CommandsResponse, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(body)?);
        return Ok(());
    }

    // フィルタ後 0 件のときは dim で告知して終了
    if body.commands.is_empty() {
        println!("  {DIM}(no commands){DIM:#}");
        return Ok(());
    }

    // パス幅は min(max(path.len()), 60)
    let path_width: usize = body
        .commands
        .iter()
        .map(|c| c.path.len())
        .max()
        .unwrap_or(0)
        .min(60);

    for c in &body.commands {
        // パラメータ列を " (name:Type, ...)" 形式で構築 (空なら空文字)
        let params: String = if c.parameters.is_empty() {
            String::new()
        } else {
            let inner = c
                .parameters
                .iter()
                .map(|p| format!("{}:{}", p.name, p.r#type))
                .collect::<Vec<_>>()
                .join(", ");
            format!(" ({})", inner)
        };

        // 説明が未設定の場合は空文字として扱う
        let description: &str = c.description.as_deref().unwrap_or("");

        // パスを左寄せ (cyan)、2 スペース空けて説明、末尾にパラメータ (dim)
        println!(
            "  {CYAN}{:<width$}{CYAN:#}  {}{DIM}{}{DIM:#}",
            c.path,
            description,
            params,
            width = path_width,
        );
    }

    println!("\n  {DIM}total: {}{DIM:#}", body.commands.len());
    Ok(())
}

pub(crate) fn render_exec(body: &ExecResponse, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(body)?);
        if !body.success {
            return Err(ExecFailure.into());
        }
        return Ok(());
    }

    // ヘッダ行: success / failed と所要時間
    if body.success {
        println!(
            "{BOLD}{GREEN}success{GREEN:#}{BOLD:#}  ({:.2} ms)",
            body.duration_ms
        );
    } else {
        println!(
            "{BOLD}{RED}failed{RED:#}{BOLD:#}  ({:.2} ms)",
            body.duration_ms
        );
    }

    // 詳細
    if let Some(v) = &body.value {
        println!("  value : {}", v);
    }
    if let Some(e) = &body.error {
        println!("  {RED}error : {}{RED:#}", e);
    }
    if let Some(et) = &body.exception_type {
        println!("  {RED}type  : {}{RED:#}", et);
    }
    if let Some(t) = &body.stack_trace {
        println!();
        println!("{DIM}{}{DIM:#}", t);
    }

    // ログ
    if !body.logs.is_empty() {
        println!();
        println!("  logs ({}):", body.logs.len());
        for l in &body.logs {
            // type に応じて色を変える (Error=red, Warning=yellow, その他=dim)
            match l.r#type.as_str() {
                "Error" => println!("    {RED}{}: {}{RED:#}", l.r#type, l.message),
                "Warning" => println!("    {YELLOW}{}: {}{YELLOW:#}", l.r#type, l.message),
                _ => println!("    {DIM}{}: {}{DIM:#}", l.r#type, l.message),
            }
        }
    }

    if !body.success {
        return Err(ExecFailure.into());
    }
    Ok(())
}

fn show_or_unknown(s: &str) -> String {
    if s.is_empty() {
        format!("{DIM}(unknown){DIM:#}")
    } else {
        s.to_string()
    }
}
