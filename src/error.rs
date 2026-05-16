use std::fmt;

/// exec / run が success: false で終わったときのエラー
/// main 側で downcast して exit code 2 に変換する。
/// 詳細メッセージは render 側で既に出力済みなので、追加表示はしない想定。
#[derive(Debug)]
pub struct ExecFailure;

impl fmt::Display for ExecFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "execution failed")
    }
}

impl std::error::Error for ExecFailure {}
