use std::{env, fs};

const TOKEN_KEY: &str = "LP_TOKEN";
const TOKEN_FILE_NAME: &str = ".liminal-palette/token";

pub fn get_token(arg_token: Option<String>) -> Result<String, &'static str> {
    // 引数で指定されていたらそれで終了
    if let Some(t) = arg_token {
        return Ok(t);
    }

    // 環境変数から読み込む
    if let Ok(t) = env::var(TOKEN_KEY) {
        return Ok(t);
    }

    // ホームディレクトリから読み込む
    let Some(mut path) = env::home_dir() else {
        return Err("ホームディレクトリが読み込めません");
    };
    path.push(TOKEN_FILE_NAME);

    if let Ok(content) = fs::read_to_string(&path) {
        return Ok(content);
    }

    return Err("トークンを読み込めませんでした");
}
