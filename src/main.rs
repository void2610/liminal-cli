use clap::Parser;
use liminal::{cli::Cli, error::ExecFailure, run};

fn main() {
    let cli = Cli::parse();
    let code = match run(cli) {
        Ok(()) => 0,
        // ExecFailure は exec/run の success: false。詳細は render 側で出力済み
        Err(e) if e.is::<ExecFailure>() => 2,
        Err(e) => {
            eprintln!("Error: {:#}", e);
            1
        }
    };
    std::process::exit(code);
}
