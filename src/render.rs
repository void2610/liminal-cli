use crate::http::HealthBody;
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

fn show_or_unknown(s: &str) -> String {
    if s.is_empty() {
        format!("{DIM}(unknown){DIM:#}")
    } else {
        s.to_string()
    }
}
