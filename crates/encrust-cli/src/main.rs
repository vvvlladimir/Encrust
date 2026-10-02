use std::process::ExitCode;

use clap::Parser;
use encrust_cli::{Args, init_tracing, run};

fn main() -> ExitCode {
    let args = Args::parse();
    init_tracing(&args);

    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
