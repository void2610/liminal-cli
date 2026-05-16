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

    for c in body.commands.clone() {
        // パラメータを構築
        let mut params: String = "".to_owned();
        for (i, p) in c.parameters.iter().enumerate() {
            params.push_str(&format!("{}:{}", p.name, p.r#type));
            if i != c.parameters.len() - 1 {
                params.push_str(", ");
            }
        }

        if params != "" {
            params = format!("({})", params);
        }

        println!(
            "{GREEN}{}{GREEN:#} {} {DIM}{}{DIM:#}",
            c.path,
            c.description.unwrap(),
            params,
        );
    }

    println!("\n{DIM}total: {}{DIM:#}", body.commands.len());
    Ok(())
}

fn show_or_unknown(s: &str) -> String {
    if s.is_empty() {
        format!("{DIM}(unknown){DIM:#}")
    } else {
        s.to_string()
    }
}
