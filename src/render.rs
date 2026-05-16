use crate::http::{CommandsBody, HealthBody};
use crate::style::{DIM, GREEN};
use anstream::println;
use anyhow::Result;

pub(crate) fn render_health(body: &HealthBody, url: &str, json: bool) -> Result<()> {
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

pub(crate) fn render_commands(body: &CommandsBody, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(body)?);
        return Ok(());
    }

    for c in &body.commands {
        // パラメータ列を "(name:Type, ...)" 形式で構築 (空なら空文字)
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

        // パスは最小幅 60 で左寄せ、その後 2 スペース空けて説明を続ける
        println!(
            "  {GREEN}{:<60}{GREEN:#}  {}{DIM}{}{DIM:#}",
            c.path, description, params,
        );
    }

    println!("\n  {DIM}total: {}{DIM:#}", body.commands.len());
    Ok(())
}

fn show_or_unknown(s: &str) -> String {
    if s.is_empty() {
        format!("{DIM}(unknown){DIM:#}")
    } else {
        s.to_string()
    }
}
