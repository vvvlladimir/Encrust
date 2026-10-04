//! `--config FILE`: flags written down in TOML, filling in what the command line left out.
//! The file's shape is in `docs/cli.md`.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::parser::ValueSource;
use clap::{ArgMatches, Command, CommandFactory, FromArgMatches, Parser};
use toml::{Table, Value};

use crate::commands::Cli;

/// Parses `args` as clap would, then once more with every flag the `--config` file names and
/// the command line does not appended, so a typed flag or its variable always wins.
pub fn parse_from(args: Vec<OsString>) -> Result<Cli, clap::Error> {
    let command = Cli::command();
    let matches = command.clone().try_get_matches_from(&args)?;
    let Some(path) = matches.get_one::<PathBuf>("config") else {
        return Cli::from_arg_matches(&matches);
    };
    let table = read(path).map_err(|message| refused(&command, &message))?;
    let mut written = Vec::new();
    fill(&command, &matches, &table, &mut written)
        .map_err(|message| refused(&command, &message))?;
    if written.is_empty() {
        return Cli::from_arg_matches(&matches);
    }
    // Anything after `--` is positional, so the written flags go in front of it.
    let at = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    let args: Vec<OsString> = [&args[..at], &written, &args[at..]].concat();
    Cli::try_parse_from(args)
}

fn read(path: &PathBuf) -> Result<Table, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    text.parse()
        .map_err(|error| format!("{} is not TOML: {error}", path.display()))
}

fn refused(command: &Command, message: &str) -> clap::Error {
    clap::Error::raw(ErrorKind::InvalidValue, format!("{message}\n")).with_cmd(command)
}

/// Writes the flags `table` holds for `command`, then descends into the table named for the
/// subcommand that was run. Tables naming other subcommands are left alone.
fn fill(
    command: &Command,
    matches: &ArgMatches,
    table: &Table,
    written: &mut Vec<OsString>,
) -> Result<(), String> {
    for (key, value) in table {
        if value.is_table() {
            continue;
        }
        let arg = command
            .get_arguments()
            .find(|arg| arg.get_long() == Some(key.as_str()))
            .ok_or_else(|| format!("`encrust {}` takes no --{key}", command.get_name()))?;
        let typed = matches!(
            matches.value_source(arg.get_id().as_str()),
            Some(ValueSource::CommandLine | ValueSource::EnvVariable)
        );
        if !typed {
            flags(key, value, written)?;
        }
    }
    let Some((name, below)) = matches.subcommand() else {
        return Ok(());
    };
    let (Some(section), Some(subcommand)) = (
        table.get(name).and_then(Value::as_table),
        command.find_subcommand(name),
    ) else {
        return Ok(());
    };
    fill(subcommand, below, section, written)
}

/// One key as the flags it stands for: `true` the bare flag, `false` nothing, an array the
/// flag once per element.
fn flags(key: &str, value: &Value, written: &mut Vec<OsString>) -> Result<(), String> {
    match value {
        Value::Boolean(true) => written.push(format!("--{key}").into()),
        Value::Boolean(false) => {}
        Value::String(text) => written.push(format!("--{key}={text}").into()),
        Value::Integer(number) => written.push(format!("--{key}={number}").into()),
        Value::Float(number) => written.push(format!("--{key}={number}").into()),
        Value::Array(values) => {
            for value in values {
                flags(key, value, written)?;
            }
        }
        Value::Datetime(_) | Value::Table(_) => {
            return Err(format!(
                "--{key} cannot be written as a {}",
                value.type_str()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Command as Sub;

    fn config(name: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("encrust-config-{name}.toml"));
        std::fs::write(&path, text).expect("the temporary directory is writable");
        path
    }

    fn parsed(args: &[&str]) -> Result<Cli, clap::Error> {
        parse_from(args.iter().map(OsString::from).collect())
    }

    #[test]
    fn a_written_flag_fills_in_what_was_not_typed_and_loses_to_what_was() {
        let path = config(
            "slice",
            "no-progress = true\n[slice]\nprinter = \"elegoo-mars-4-ultra\"\ncenter = true\n\
             [batch]\njobs = 3\n",
        );
        let path = path.to_str().expect("a UTF-8 temporary path");
        let cli = parsed(&[
            "encrust",
            "--config",
            path,
            "slice",
            "a.stl",
            "--printer",
            "x",
        ])
        .expect("the file and the line agree on every flag");
        assert!(
            cli.global.no_progress,
            "a global flag at the top of the file"
        );
        let Sub::Slice(slice) = cli.command else {
            panic!("slice was run");
        };
        assert_eq!(
            slice.job.profile.printer.as_deref(),
            Some("x"),
            "typed wins"
        );
        assert!(slice.job.import.transform.center, "written fills in");
    }

    #[test]
    fn a_key_the_command_does_not_take_is_refused() {
        let path = config("unknown", "[slice]\nno-such-flag = 1\n");
        let path = path.to_str().expect("a UTF-8 temporary path");
        let error = parsed(&["encrust", "--config", path, "slice", "a.stl"])
            .expect_err("an unknown key is an argument error");
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn a_file_that_is_not_toml_is_refused() {
        let path = config("broken", "[slice\n");
        let path = path.to_str().expect("a UTF-8 temporary path");
        let error = parsed(&["encrust", "--config", path, "inspect", "a.stl"])
            .expect_err("a broken file is an argument error");
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }
}
