use anyhow::Result;
use clap::Parser;
use liminal::{cli::Cli, run};

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}
