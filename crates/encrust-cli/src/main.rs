use std::ffi::OsString;
use std::process::ExitCode;

use encrust_cli::{Stop, exit_code, init_tracing, json_error, parse_from, run};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let wants_json = args
        .iter()
        .skip(1)
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json");
    let cli = parse_from(args).unwrap_or_else(|error| {
        if wants_json && error.use_stderr() {
            println!("{}", json_error(&usage_error(&error)));
        }
        error.exit()
    });
    init_tracing(&cli);

    // The first Ctrl-C lets the run stop between layers and remove what it half wrote; a
    // second one does not wait.
    let stop = Stop::default();
    let handler = stop.clone();
    if cli.command.watches_stop()
        && let Err(error) = ctrlc::set_handler(move || {
            if handler.request() {
                std::process::exit(130);
            }
        })
    {
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

/// clap's complaint as an error document carries it: its first line, without the usage
/// that follows for a reader at a terminal.
fn usage_error(error: &clap::Error) -> anyhow::Error {
    let rendered = error.render().to_string();
    let line = rendered.lines().next().unwrap_or_default();
    anyhow::anyhow!("{}", line.strip_prefix("error: ").unwrap_or(line))
}
