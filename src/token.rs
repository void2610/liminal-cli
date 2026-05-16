use std::{env, fs};

const TOKEN_KEY: &str = "LP_TOKEN";
const TOKEN_FILE_NAME: &str = ".liminal-palette/token";

/// SPEC §2 のトークン解決:
/// 1. `--token` 明示 (strip 後に空でなければ採用)
/// 2. `$LP_TOKEN` (strip 後に空でなければ採用)
/// 3. `~/.liminal-palette/token` の中身 (strip 後に空でなければ採用)
/// 4. なし → `None`
pub fn get_token(arg_token: Option<String>) -> Option<String> {
    // 1. --token
    if let Some(t) = arg_token {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }

    // 2. $LP_TOKEN
    if let Ok(t) = env::var(TOKEN_KEY) {
        let t = t.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }

    // 3. ~/.liminal-palette/token
    let mut path = env::home_dir()?;
    path.push(TOKEN_FILE_NAME);
    if let Ok(content) = fs::read_to_string(&path) {
        let t = content.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }

    None
}
