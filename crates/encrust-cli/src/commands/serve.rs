//! `encrust serve`: one process that runs many invocations, each asked for as a line of JSON on
//! stdin and answered as one on stdout, keeping what one run made for the next (ADR 0467).

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::{Command, run};
use crate::exit::{Exit, Stop, exit_code};
use crate::steps::Steps;
use crate::{json, parse_from, usage_error};

#[derive(clap::Args, Debug)]
pub struct ServeCommand {}

/// A line read from stdin: a run to start, or one to stop.
#[derive(Deserialize)]
#[serde(untagged)]
enum Asked {
    /// The command line's words after `encrust`, `--json` taken as given.
    Run {
        id: u64,
        args: Vec<String>,
    },
    Cancel {
        cancel: u64,
    },
}

/// The line a run is answered with: its exit code and the one document it printed.
#[derive(Serialize)]
struct Answer {
    id: u64,
    exit_code: u8,
    report: Value,
}

/// The runs going, each with the stop it reads.
type Going = Arc<Mutex<HashMap<u64, Stop>>>;
/// Where answers are written, one whole line at a time.
type Out = Arc<Mutex<dyn Write + Send>>;

/// A run's stack, bytes: the main thread's a run of its own has, not a spawned thread's 2 MiB.
const STACK: usize = 8 << 20;

impl ServeCommand {
    pub fn run(&self) -> Result<Exit> {
        let steps = Arc::new(Steps::default());
        let going: Going = Arc::default();
        let out: Out = Arc::new(Mutex::new(std::io::stdout()));
        let mut threads: Vec<JoinHandle<()>> = Vec::new();
        for line in std::io::stdin().lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Asked>(&line) {
                Ok(Asked::Run { id, args }) => {
                    threads.retain(|thread| !thread.is_finished());
                    threads.extend(started(id, args, (&steps, &going, &out)));
                }
                Ok(Asked::Cancel { cancel }) => {
                    if let Some(stop) = lock(&going).get(&cancel) {
                        stop.request();
                    }
                }
                Err(error) => tracing::warn!("serve: a line that asks nothing, {error}: {line}"),
            }
        }
        for stop in lock(&going).values() {
            stop.request();
        }
        for thread in threads {
            let _ = thread.join();
        }
        Ok(Exit::Success)
    }
}

/// Starts run `id` on a thread of its own, answered on `out` when it ends; one whose id is
/// still going is answered as failed at once.
fn started(
    id: u64,
    args: Vec<String>,
    (steps, going, out): (&Arc<Steps>, &Going, &Out),
) -> Option<JoinHandle<()>> {
    let stop = Stop::default();
    let fresh = match lock(going).entry(id) {
        std::collections::hash_map::Entry::Occupied(_) => false,
        std::collections::hash_map::Entry::Vacant(free) => {
            free.insert(stop.clone());
            true
        }
    };
    if !fresh {
        answer(out, &failed(id, &anyhow!("run {id} is still going")));
        return None;
    }
    let run = (Arc::clone(steps), Arc::clone(going), Arc::clone(out));
    let spawned = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || {
            let (steps, going, out) = run;
            let answered = answered(id, &args, &stop, steps);
            lock(&going).remove(&id);
            answer(&out, &answered);
        });
    match spawned {
        Ok(thread) => Some(thread),
        Err(error) => {
            lock(going).remove(&id);
            answer(out, &failed(id, &anyhow!("cannot start the run: {error}")));
            None
        }
    }
}

/// Writes `answer` as one line of `out`.
fn answer(out: &Out, answer: &Answer) {
    let mut out = lock(out);
    let _ = serde_json::to_writer(&mut *out, answer);
    let _ = writeln!(out).and_then(|()| out.flush());
}

/// Runs the invocation `args` name with `steps` kept, and answers it as `main` would have.
fn answered(id: u64, args: &[String], stop: &Stop, steps: Arc<Steps>) -> Answer {
    let words = ["encrust", "--json"].into_iter().map(OsString::from);
    let words: Vec<OsString> = words.chain(args.iter().map(OsString::from)).collect();
    let mut cli = match parse_from(words) {
        Ok(cli) => cli,
        Err(error) => return failed(id, &usage_error(&error)),
    };
    if matches!(cli.command, Command::Serve(_)) {
        return failed(id, &anyhow!("serve runs the other commands, not itself"));
    }
    cli.global.json = true;
    cli.global.steps = Some(steps);
    let (result, printed) = json::caught(|| {
        catch_unwind(AssertUnwindSafe(|| run(&cli, stop)))
            .unwrap_or_else(|panic| Err(anyhow!("the run failed: {}", said(&panic))))
    });
    let report = match (&result, printed) {
        (Ok(_), Some(printed)) => serde_json::from_str(&printed).unwrap_or(Value::Null),
        (Err(error), _) => serde_json::from_str(&json::error(error)).unwrap_or(Value::Null),
        (Ok(_), None) => Value::Null,
    };
    Answer {
        id,
        exit_code: exit_code(&result),
        report,
    }
}

fn failed(id: u64, error: &anyhow::Error) -> Answer {
    Answer {
        id,
        exit_code: exit_code(&Err(anyhow!("{error}"))),
        report: serde_json::from_str(&json::error(error)).unwrap_or(Value::Null),
    }
}

/// What a panic said, where it said it as text.
fn said(panic: &Box<dyn std::any::Any + Send>) -> String {
    (panic.downcast_ref::<&str>().map(|said| (*said).to_owned()))
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".to_owned())
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
