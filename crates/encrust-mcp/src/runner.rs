//! Runs the `encrust` command line in one `encrust serve` it keeps going, a line of JSON a run,
//! and reads the one document each run prints (`docs/cli.md`, "Serve"; ADR 0467).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

/// The exit code `--strict` ends a run with when a model has defects or does not fit.
pub const DEFECTS: i32 = 3;

/// How long a run asked to stop may take to stop before its server is ended with it.
const GRACE: Duration = Duration::from_secs(15);
/// The lines of the server's stderr kept, said when it ends.
const STDERR_LINES: usize = 20;

/// The `encrust` binary this server drives, and how long one run may take; a clone drives the
/// same `encrust serve`.
#[derive(Debug, Clone)]
pub struct Cli {
    program: PathBuf,
    timeout: Duration,
    worker: Arc<tokio::sync::Mutex<Option<Arc<Worker>>>>,
}

/// What one run of the command line said.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit_code: i32,
    pub report: Value,
}

impl Outcome {
    /// Whether the run ended in code 0, or in defects it reported.
    pub fn reported(&self) -> bool {
        self.exit_code == 0 || self.exit_code == DEFECTS
    }
}

/// A line `encrust serve` answers with.
#[derive(Deserialize)]
struct Answer {
    id: u64,
    exit_code: i32,
    report: Value,
}

/// The runs waiting for their answer, by id.
type Waiting = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Outcome, String>>>>>;

/// One `encrust serve` and the runs it has been asked for.
#[derive(Debug)]
struct Worker {
    asks: mpsc::UnboundedSender<String>,
    waiting: Waiting,
    next: AtomicU64,
    alive: Arc<AtomicBool>,
    child: Mutex<Child>,
}

impl Cli {
    pub fn new(program: PathBuf, timeout: Duration) -> Self {
        Self {
            program,
            timeout,
            worker: Arc::default(),
        }
    }

    /// How long one run may take.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The `encrust` beside the server's own binary, as a bundle ships them, else the one on
    /// the `PATH`.
    pub fn program_beside(server: &Path) -> PathBuf {
        let name = format!("encrust{}", std::env::consts::EXE_SUFFIX);
        server
            .parent()
            .map(|dir| dir.join(&name))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(name))
    }

    /// Runs `encrust --json <args>`. A run that printed a report is an `Outcome` whatever its
    /// exit code; one that printed none, or ran past the time limit, is an error. Dropping the
    /// future stops the run.
    pub async fn run(&self, args: &[String]) -> Result<Outcome> {
        let worker = self.worker().await?;
        let (id, mut answer) = worker.ask(args)?;
        let mut asked = Asked {
            worker: &worker,
            id,
            answered: false,
        };
        let Ok(answered) = tokio::time::timeout(self.timeout, &mut answer).await else {
            worker.cancel(id);
            if tokio::time::timeout(GRACE, &mut answer).await.is_err() {
                worker.end();
            }
            asked.answered = true;
            bail!(
                "encrust ran past {} s and was stopped",
                self.timeout.as_secs()
            );
        };
        asked.answered = true;
        let outcome = answered
            .map_err(|_| anyhow::anyhow!("encrust stopped before it answered"))?
            .map_err(|why| anyhow::anyhow!("{why}"))?;
        if outcome.report.is_null() {
            bail!(
                "encrust exited with code {} and no report",
                outcome.exit_code
            );
        }
        Ok(outcome)
    }

    /// The server going, started anew where none is or the last one ended.
    async fn worker(&self) -> Result<Arc<Worker>> {
        let mut held = self.worker.lock().await;
        let going = held
            .as_ref()
            .filter(|worker| worker.alive.load(Ordering::Relaxed));
        if let Some(worker) = going {
            return Ok(Arc::clone(worker));
        }
        let worker = Arc::new(Worker::start(&self.program)?);
        *held = Some(Arc::clone(&worker));
        Ok(worker)
    }
}

/// A run asked for, which is asked to stop if it is dropped before its answer came.
struct Asked<'a> {
    worker: &'a Worker,
    id: u64,
    answered: bool,
}

impl Drop for Asked<'_> {
    fn drop(&mut self) {
        if !self.answered {
            self.worker.cancel(self.id);
        }
    }
}

impl Worker {
    /// Starts `program serve`, with a task writing what it is asked and one reading its answers.
    fn start(program: &Path) -> Result<Self> {
        let mut child = Command::new(program)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("starting {}", program.display()))?;
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            bail!("{} started without its pipes", program.display());
        };
        let (asks, asked) = mpsc::unbounded_channel::<String>();
        tokio::spawn(written(stdin, asked));
        let said = Arc::new(Mutex::new(VecDeque::new()));
        tokio::spawn(kept_stderr(stderr, Arc::clone(&said)));
        let waiting: Waiting = Arc::default();
        let alive = Arc::new(AtomicBool::new(true));
        let reading = (Arc::clone(&waiting), Arc::clone(&alive), said);
        tokio::spawn(answers(stdout, reading));
        Ok(Self {
            asks,
            waiting,
            next: AtomicU64::new(1),
            alive,
            child: Mutex::new(child),
        })
    }

    /// Sends the run `args` name, and where its answer will come.
    fn ask(&self, args: &[String]) -> Result<(u64, oneshot::Receiver<Result<Outcome, String>>)> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (answer, answered) = oneshot::channel();
        lock(&self.waiting).insert(id, answer);
        let line = format!("{}\n", json!({"id": id, "args": args}));
        if self.asks.send(line).is_err() || !self.alive.load(Ordering::Relaxed) {
            lock(&self.waiting).remove(&id);
            bail!("encrust stopped and could not be asked");
        }
        Ok((id, answered))
    }

    /// Asks run `id` to stop between its steps.
    fn cancel(&self, id: u64) {
        let _ = self.asks.send(format!("{}\n", json!({ "cancel": id })));
    }

    /// Ends the server, every run in it with it; the next run starts another.
    fn end(&self) {
        self.alive.store(false, Ordering::Relaxed);
        let _ = lock(&self.child).start_kill();
    }
}

/// Writes each line asked to the server's stdin, until it closes.
async fn written(
    mut stdin: tokio::process::ChildStdin,
    mut asked: mpsc::UnboundedReceiver<String>,
) {
    while let Some(line) = asked.recv().await {
        let sent = stdin.write_all(line.as_bytes()).await;
        if sent.is_err() || stdin.flush().await.is_err() {
            break;
        }
    }
}

/// Hands each answer read off `stdout` to the run waiting for it; once the server ends, every
/// run still waiting is told so, with the last of what it said on stderr.
async fn answers(
    stdout: tokio::process::ChildStdout,
    (waiting, alive, said): (Waiting, Arc<AtomicBool>, Arc<Mutex<VecDeque<String>>>),
) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(answer) = serde_json::from_str::<Answer>(&line) else {
            continue;
        };
        if let Some(waiting) = lock(&waiting).remove(&answer.id) {
            let _ = waiting.send(Ok(Outcome {
                exit_code: answer.exit_code,
                report: answer.report,
            }));
        }
    }
    alive.store(false, Ordering::Relaxed);
    let said: Vec<String> = lock(&said).iter().cloned().collect();
    let why = format!("encrust stopped: {}", said.join("\n").trim());
    for (_, waiting) in lock(&waiting).drain() {
        let _ = waiting.send(Err(why.clone()));
    }
}

/// Keeps the last lines the server wrote to stderr.
async fn kept_stderr(stderr: tokio::process::ChildStderr, said: Arc<Mutex<VecDeque<String>>>) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut said = lock(&said);
        said.push_back(line);
        if said.len() > STDERR_LINES {
            said.pop_front();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
