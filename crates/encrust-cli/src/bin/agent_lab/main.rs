//! A bench for language models preparing a print: each model prepared by the engine's autopilot
//! and by each chat client under test over `encrust-mcp`, scored alike. See `docs/cli.md`.

mod agent;
mod corpus;
mod judge;
mod report;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde_json::Value;

use crate::agent::{Agent, Left, Programs, run_bounded};
use crate::judge::{Drawn, Judge};
use crate::report::{AgentRow, ModelRow, Prepared, Report};
use encrust_cli::score::{Score, judge};

/// What every client is asked, the model, printer and resin filled in.
const PROMPT: &str = "Prepare the model {model} for printing on the printer {printer} with the \
resin {resin}, using the encrust tools. The user chose this printer and resin and is away: \
ask nothing, write no file and send nothing to a printer. Stop when the check is as clean as \
you can make it; the job's latest revision is your answer.";

/// Prepares each model by the autopilot and by each agent, and scores both.
#[derive(Parser)]
#[command(name = "agent-lab")]
struct Args {
    /// Models to prepare.
    models: Vec<PathBuf>,
    /// A corpus file listing the models.
    #[arg(long)]
    corpus: Option<PathBuf>,
    /// Prepare only the models whose name holds this.
    #[arg(long)]
    only: Option<String>,
    /// A printer id of the catalogue, or a printer TOML.
    #[arg(long, default_value = "elegoo-saturn-4-ultra")]
    printer: String,
    /// A resin id of the user's, or a resin TOML.
    #[arg(long)]
    resin: String,
    /// A client under test as NAME=COMMAND; repeat for several. None benches the autopilot alone.
    #[arg(long = "agent", value_name = "NAME=COMMAND")]
    agents: Vec<String>,
    /// How long one client or one check may take, seconds.
    #[arg(long, default_value_t = 3600)]
    timeout_s: u64,
    /// Where each run keeps its jobs, trace and output; a directory under the system's temp
    /// directory if unset.
    #[arg(long)]
    work: Option<PathBuf>,
    /// The `encrust` the checks run; the one beside this binary if unset.
    #[arg(long)]
    encrust: Option<PathBuf>,
    /// The `encrust-mcp` the clients start; the one beside this binary if unset.
    #[arg(long)]
    encrust_mcp: Option<PathBuf>,
    /// Write the run as JSON, to keep as a baseline.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Compare with a run kept before; an agent that held there and does not here, or an
    /// autopilot worse than it was, fails the run.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Draw every preparation from the front and from below, beside its run.
    #[arg(long)]
    pictures: bool,
    /// A judge, as COMMAND with `{prompt}`, shown both preparations drawn and asked which
    /// prints better; another model than the clients', so none grades its own work.
    #[arg(long, value_name = "COMMAND")]
    judge: Option<String>,
}

/// One model to prepare.
struct Subject {
    name: String,
    path: PathBuf,
    skip: Option<String>,
}

fn main() -> Result<ExitCode> {
    let mut args = Args::parse();
    for profile in [&mut args.printer, &mut args.resin] {
        *profile = absolute_toml(profile)?;
    }
    let agents = args
        .agents
        .iter()
        .map(|spec| Agent::parse(spec))
        .collect::<Result<Vec<_>>>()?;
    let judge = args.judge.as_deref().map(Judge::parse).transpose()?;
    let programs = Programs {
        encrust: program(args.encrust.as_deref(), "encrust")?,
        encrust_mcp: program(args.encrust_mcp.as_deref(), "encrust-mcp")?,
        timeout: Duration::from_secs(args.timeout_s),
    };
    let work = args.work.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("encrust-agent-lab-{}", std::process::id()))
    });
    println!(
        "printer {}, resin {}, {} agents, work in {}\n",
        args.printer,
        args.resin,
        agents.len(),
        work.display()
    );
    let mut report = Report {
        printer: args.printer.clone(),
        resin: args.resin.clone(),
        models: Vec::new(),
    };
    for subject in subjects(&args)? {
        let row = bench(
            &subject,
            (&args, judge.as_ref()),
            &agents,
            &programs,
            &work.join(&subject.name),
        );
        report::print_model(&row);
        report.models.push(row);
    }
    let (held, judged) = report.tally();
    println!("{held} of {judged} agent preparations match or beat the autopilot");

    if let Some(path) = &args.json {
        report.write(path)?;
    }
    if let Some(path) = &args.baseline {
        let changes = report::compare(&Report::read(path)?, &report);
        report::print_changes(&changes);
        if changes.iter().any(report::Change::regressed) {
            return Ok(ExitCode::FAILURE);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// A program named outright, or the one built beside this binary.
fn program(named: Option<&Path>, name: &str) -> Result<PathBuf> {
    if let Some(path) = named {
        return Ok(path.to_owned());
    }
    let lab = std::env::current_exe().context("finding this bench's own binary")?;
    let beside = lab.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    if !beside.is_file() {
        bail!(
            "no {name} beside {}: build it with the same profile, or pass --{name}",
            lab.display()
        );
    }
    Ok(beside)
}

/// The models the arguments name, filtered by `--only`, each made absolute as the server
/// requires.
fn subjects(args: &Args) -> Result<Vec<Subject>> {
    let mut subjects: Vec<Subject> = match &args.corpus {
        Some(path) => corpus::read(path)?
            .into_iter()
            .map(|entry| Subject {
                name: entry.name,
                path: entry.path,
                skip: entry.skip,
            })
            .collect(),
        None => args
            .models
            .iter()
            .map(|path| Subject {
                name: name_of(path),
                path: path.clone(),
                skip: None,
            })
            .collect(),
    };
    if subjects.is_empty() {
        bail!("name models to prepare, or a --corpus");
    }
    if let Some(only) = &args.only {
        subjects.retain(|subject| subject.name.contains(only.as_str()));
    }
    for subject in &mut subjects {
        if subject.skip.is_none() {
            subject.path = std::path::absolute(&subject.path)
                .with_context(|| format!("cannot make {} absolute", subject.path.display()))?;
        }
    }
    Ok(subjects)
}

fn name_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// One model, prepared by the autopilot and then by each agent. A run that cannot start is
/// a row with no score, so a corpus still finishes.
fn bench(
    subject: &Subject,
    (args, judge): (&Args, Option<&Judge>),
    agents: &[Agent],
    programs: &Programs,
    work: &Path,
) -> ModelRow {
    let mut row = ModelRow {
        name: subject.name.clone(),
        skipped: subject.skip.clone(),
        autopilot: unscored(String::new(), 0.0),
        agents: Vec::new(),
    };
    if subject.skip.is_some() {
        return row;
    }
    let drawing = args.pictures || judge.is_some();
    let flags = autopilot_flags(subject, args);
    row.autopilot = autopilot(&flags, programs, &work.join("autopilot"))
        .unwrap_or_else(|error| unscored(format!("{error:#}"), 0.0));
    if drawing {
        row.autopilot.pictures = drawn(&flags, &work.join("autopilot/pictures"), programs);
    }
    let prompt = PROMPT
        .replace("{model}", &subject.path.display().to_string())
        .replace("{printer}", &args.printer)
        .replace("{resin}", &args.resin);
    for agent in agents {
        let work = work.join(&agent.name);
        let mut agent_row = match agent.run(&prompt, &work, programs) {
            Ok(left) => left_row(agent, left, &row.autopilot, (drawing, &work, programs)),
            Err(error) => AgentRow {
                agent: agent.name.clone(),
                prepared: unscored(format!("{error:#}"), 0.0),
                tool_calls: 0,
                failed_calls: 0,
                tokens: None,
                judged: None,
                verdict: None,
            },
        };
        if let Some(judge) = judge {
            agent_row.verdict = ruled(
                judge,
                &subject.name,
                (&row.autopilot, &agent_row),
                &work,
                programs,
            );
        }
        row.agents.push(agent_row);
    }
    row
}

/// What a client left, scored and set against the autopilot's `autopilot`, drawn if asked.
fn left_row(
    agent: &Agent,
    left: Left,
    autopilot: &Prepared,
    (drawing, work, programs): (bool, &Path, &Programs),
) -> AgentRow {
    let mut prepared = match left.check.as_ref().map(Score::of) {
        Some(Ok(score)) => scored(score, left.seconds),
        Some(Err(error)) => unscored(format!("{error:#}"), left.seconds),
        None => unscored(left.why_not.unwrap_or_default(), left.seconds),
    };
    if let (true, Some(flags)) = (drawing, &left.args) {
        prepared.pictures = drawn(flags, &work.join("pictures"), programs);
    }
    AgentRow {
        agent: agent.name.clone(),
        judged: prepared
            .score
            .zip(autopilot.score)
            .map(|(mine, theirs)| judge(&mine, &theirs)),
        prepared,
        tool_calls: left.tool_calls,
        failed_calls: left.failed_calls,
        tokens: left.tokens,
        verdict: None,
    }
}

/// The pictures of a preparation, or none and why on stderr: a run goes on without them.
fn drawn(flags: &[String], dir: &Path, programs: &Programs) -> Vec<PathBuf> {
    judge::draw(flags, dir, programs).unwrap_or_else(|error| {
        eprintln!("not drawn: {error:#}");
        Vec::new()
    })
}

/// The judge's verdict on a client's preparation against the autopilot's, where both are
/// drawn; none, and why on stderr, where it gave none.
fn ruled(
    judge: &Judge,
    model: &str,
    (autopilot, agent): (&Prepared, &AgentRow),
    work: &Path,
    programs: &Programs,
) -> Option<judge::Verdict> {
    if autopilot.pictures.is_empty() || agent.prepared.pictures.is_empty() {
        return None;
    }
    let drawn = |prepared: &Prepared| {
        (
            prepared.pictures.clone(),
            judge::shown(prepared.score.as_ref()),
        )
    };
    let (a, b) = (drawn(autopilot), drawn(&agent.prepared));
    let shown = (
        Drawn {
            pictures: &a.0,
            score: &a.1,
        },
        Drawn {
            pictures: &b.0,
            score: &b.1,
        },
    );
    judge
        .rule(model, shown, &work.join("judge"), programs)
        .inspect_err(|error| eprintln!("no verdict: {error:#}"))
        .ok()
}

/// The flags that stage the model under `--prepare`, as `start_job` does with no step
/// overridden.
fn autopilot_flags(subject: &Subject, args: &Args) -> Vec<String> {
    vec![
        subject.path.display().to_string(),
        profile_flag(&args.printer, "--printer", "--profile"),
        profile_flag(&args.resin, "--resin", "--material"),
        "--center".into(),
        "--check-drainage".into(),
        "--prepare".into(),
    ]
}

/// The model checked as `flags` stage it.
fn autopilot(flags: &[String], programs: &Programs, work: &Path) -> Result<Prepared> {
    std::fs::create_dir_all(work).with_context(|| format!("creating {}", work.display()))?;
    let mut command = Command::new(&programs.encrust);
    command.args(["--json", "estimate"]).args(flags);
    let stdout = work.join("report.json");
    let started = Instant::now();
    let status = run_bounded(command, &stdout, &work.join("stderr.txt"), programs.timeout)?;
    let seconds = started.elapsed().as_secs_f64();
    if status.is_none() {
        return Ok(unscored(
            format!("ran past {} s", programs.timeout.as_secs()),
            seconds,
        ));
    }
    let text = std::fs::read_to_string(&stdout)?;
    let report: Value = serde_json::from_str(text.trim())
        .with_context(|| format!("encrust printed no report; see {}", work.display()))?;
    Ok(match Score::of(&report) {
        Ok(score) => scored(score, seconds),
        Err(error) => unscored(format!("{error:#}"), seconds),
    })
}

/// A profile TOML made absolute, since each client runs in a directory of its own; an id as
/// it is.
fn absolute_toml(named: &str) -> Result<String> {
    if !is_toml(named) {
        return Ok(named.to_owned());
    }
    let path =
        std::path::absolute(named).with_context(|| format!("cannot make {named} absolute"))?;
    Ok(path.display().to_string())
}

fn is_toml(named: &str) -> bool {
    Path::new(named)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}

/// A catalogue id takes its own flag, a TOML file the path flag, as the server passes them.
fn profile_flag(named: &str, by_id: &str, by_path: &str) -> String {
    format!("{}={named}", if is_toml(named) { by_path } else { by_id })
}

fn scored(score: Score, seconds: f64) -> Prepared {
    Prepared {
        score: Some(score),
        why_not: None,
        seconds,
        pictures: Vec::new(),
    }
}

fn unscored(why: String, seconds: f64) -> Prepared {
    Prepared {
        score: None,
        why_not: Some(why),
        seconds,
        pictures: Vec::new(),
    }
}
