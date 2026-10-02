use std::process::ExitCode;

use clap::Parser;
use encrust_cli::{Cli, Stop, exit_code, init_tracing, json_error, run};

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(&cli);

    // The first Ctrl-C lets the run stop between layers and remove what it half wrote; a
    // second one does not wait.
    let stop = Stop::default();
    let handler = stop.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        if handler.request() {
            std::process::exit(130);
        }
    }) {
        tracing::warn!("Ctrl-C will not stop the run cleanly: {error}");
    }

    let result = run(&cli, &stop);
    if let Err(error) = &result {
        eprintln!("error: {error:#}");
        if cli.global.json {
            println!("{}", json_error(error));
        }
    }
    ExitCode::from(exit_code(&result))
}
