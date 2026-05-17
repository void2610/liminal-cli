use std::{env, fs};

const TOKEN_KEY: &str = "LP_TOKEN";
const TOKEN_FILE_NAME: &str = ".liminal-palette/token";

/// SPEC §2 のトークン解決:
/// 1. `--token` 明示 (strip 後に空でなければ採用)
/// 2. `$LP_TOKEN` (strip 後に空でなければ採用)
/// 3. `~/.liminal-palette/token` の中身 (strip 後に空でなければ採用)
/// 4. なし → `None`
pub fn get_token(arg_token: Option<String>) -> Option<String> {
    let env_token = env::var(TOKEN_KEY).ok();
    let file_token = read_token_file();
    resolve_token(arg_token, env_token, file_token)
}

/// I/O を分離した純粋ロジック。priority は arg > env > file
pub(crate) fn resolve_token(
    arg: Option<String>,
    env: Option<String>,
    file: Option<String>,
) -> Option<String> {
    for candidate in [arg, env, file].into_iter().flatten() {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn read_token_file() -> Option<String> {
    let mut path = env::home_dir()?;
    path.push(TOKEN_FILE_NAME);
    fs::read_to_string(&path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_arg_が_最優先() {
        let r = resolve_token(
            Some("arg".into()),
            Some("env".into()),
            Some("file".into()),
        );
        assert_eq!(r, Some("arg".into()));
    }

    #[test]
    fn token_arg_が_空白のみなら_env_にフォールバック() {
        let r = resolve_token(Some("  \n".into()), Some("env".into()), None);
        assert_eq!(r, Some("env".into()));
    }

    #[test]
    fn token_env_は_file_より優先() {
        let r = resolve_token(None, Some("env".into()), Some("file".into()));
        assert_eq!(r, Some("env".into()));
    }

    #[test]
    fn token_env_が_空白のみなら_file_にフォールバック() {
        let r = resolve_token(None, Some("   ".into()), Some("file".into()));
        assert_eq!(r, Some("file".into()));
    }

    #[test]
    fn token_file_の改行は_strip_される() {
        let r = resolve_token(None, None, Some("abc\n".into()));
        assert_eq!(r, Some("abc".into()));
    }

    #[test]
    fn token_前後の空白は_strip_される() {
        let r = resolve_token(None, None, Some("  abc  \n".into()));
        assert_eq!(r, Some("abc".into()));
    }

    #[test]
    fn token_すべて_未設定なら_none() {
        let r = resolve_token(None, None, None);
        assert_eq!(r, None);
    }

    #[test]
    fn token_すべて_空白のみなら_none() {
        let r = resolve_token(Some("".into()), Some("  ".into()), Some("\t\n".into()));
        assert_eq!(r, None);
    }
}
