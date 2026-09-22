use anstream::eprintln;
use clap::Parser;
use liminal::{cli::Cli, error::ExecFailure, run, style::RED};

fn main() {
    // clap の既定はパースエラーで exit 2 だが、SPEC §11 では 2 は「実行はされたが失敗」の意味。
    // 引数エラーは 1 に倒し、--help / --version は 0 のままにする。
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() { 1 } else { 0 };
            let _ = e.print();
            std::process::exit(code);
        }
    };
    let code = match run(cli) {
        Ok(()) => 0,
        // ExecFailure は exec/run の success: false。詳細は render 側で出力済み
        Err(e) if e.is::<ExecFailure>() => 2,
        Err(e) => {
            // SPEC §10: 致命エラーは赤で出す (非 TTY / NO_COLOR では anstream が色を落とす)
            eprintln!("{RED}Error: {:#}{RED:#}", e);
            1
        }
    };
    std::process::exit(code);
}
