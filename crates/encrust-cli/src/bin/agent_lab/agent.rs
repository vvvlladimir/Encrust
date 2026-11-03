//! A chat client under test, run as its own command against `encrust-mcp`, and what it left:
//! the trace of its calls, the job it prepared, and the tokens it says it spent.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// How often a running command is looked at; its time limit is what bounds the wait.
const POLL: Duration = Duration::from_millis(200);

/// One client, by the name the report gives it and the command that runs it, split at
/// whitespace, each word with its placeholders filled in (`docs/cli.md`, `agent-lab`).
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    pub name: String,
    pub words: Vec<String>,
}

/// The programs a run drives, and how long one command may take.
pub struct Programs {
    pub encrust: PathBuf,
    pub encrust_mcp: PathBuf,
    pub timeout: Duration,
}

/// What one client left behind.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Left {
    pub seconds: f64,
    pub tool_calls: usize,
    pub failed_calls: usize,
    pub tokens: Option<u64>,
    /// The check of its job's latest revision, if it started one.
    pub check: Option<Value>,
    /// Why there is no check to score.
    pub why_not: Option<String>,
}

impl Agent {
    /// `NAME=COMMAND`, as `--agent` takes it.
    pub fn parse(spec: &str) -> Result<Self> {
        let Some((name, command)) = spec.split_once('=') else {
            bail!("an agent is NAME=COMMAND, not {spec:?}");
        };
        let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        if name.trim().is_empty() || words.is_empty() {
            bail!("an agent needs a name and a command: {spec:?}");
        }
        Ok(Self {
            name: name.trim().to_owned(),
            words,
        })
    }

    /// Runs the client in `work` with `prompt`, its server keeping jobs and trace there.
    pub fn run(&self, prompt: &str, work: &Path, programs: &Programs) -> Result<Left> {
        let _ = fs::remove_dir_all(work);
        let jobs = work.join("jobs");
        fs::create_dir_all(&jobs).with_context(|| format!("creating {}", jobs.display()))?;
        let trace = work.join("trace.jsonl");
        let config = work.join("mcp.json");
        let server_args = [
            "--encrust".to_owned(),
            programs.encrust.display().to_string(),
            "--jobs".to_owned(),
            jobs.display().to_string(),
            "--trace".to_owned(),
            trace.display().to_string(),
        ];
        let server = json!({ "mcpServers": { "encrust": {
            "command": programs.encrust_mcp, "args": server_args,
        }}});
        fs::write(&config, server.to_string())
            .with_context(|| format!("writing {}", config.display()))?;
        let fill = |word: &str| {
            word.replace("{prompt}", prompt)
                .replace("{mcp_config}", &config.display().to_string())
                .replace("{encrust_mcp}", &programs.encrust_mcp.display().to_string())
                .replace("{mcp_args}", &json!(server_args).to_string())
                .replace("{work}", &work.display().to_string())
        };
        let words: Vec<String> = self.words.iter().map(|word| fill(word)).collect();
        let stdout = work.join("stdout.txt");
        let started = Instant::now();
        let mut command = Command::new(&words[0]);
        command.args(&words[1..]).current_dir(work);
        let status = run_bounded(command, &stdout, &work.join("stderr.txt"), programs.timeout)?;
        let mut left = Left {
            seconds: started.elapsed().as_secs_f64(),
            tokens: tokens_in(&fs::read_to_string(&stdout).unwrap_or_default()),
            ..Left::default()
        };
        let calls = read_trace(&trace)?;
        left.tool_calls = calls.len();
        left.failed_calls = calls.iter().filter(|call| call["failed"] == true).count();
        let last_job = calls.iter().rev().find_map(|call| call["job"].as_str());
        match (status, last_job) {
            (None, _) => left.why_not = Some(format!("ran past {} s", programs.timeout.as_secs())),
            (_, None) => left.why_not = Some("started no job".into()),
            (Some(_), Some(job)) => match latest_check(&jobs.join(job)) {
                Ok(check) => left.check = Some(check),
                Err(error) => left.why_not = Some(format!("{error:#}")),
            },
        }
        Ok(left)
    }
}

/// Runs `command` with its output in the two files, killing it past `timeout`; `None` when it
/// was killed.
pub fn run_bounded(
    mut command: Command,
    stdout: &Path,
    stderr: &Path,
    timeout: Duration,
) -> Result<Option<ExitStatus>> {
    let create =
        |path: &Path| File::create(path).with_context(|| format!("creating {}", path.display()));
    let mut child: Child = command
        .stdin(Stdio::null())
        .stdout(create(stdout)?)
        .stderr(create(stderr)?)
        .spawn()
        .with_context(|| format!("starting {:?}", command.get_program()))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        std::thread::sleep(POLL);
    }
}

/// The lines of a trace; a client that called nothing leaves none.
fn read_trace(path: &Path) -> Result<Vec<Value>> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).with_context(|| format!("a trace line {line:?}")))
        .collect()
}

/// The kept check of a job's latest revision: the highest `N.toml`'s `N.json`.
fn latest_check(job: &Path) -> Result<Value> {
    let latest = fs::read_dir(job)
        .with_context(|| format!("reading the job {}", job.display()))?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "toml").then_some(())?;
            path.file_stem()?.to_str()?.parse::<u32>().ok()
        })
        .max()
        .with_context(|| format!("the job {} has no revision", job.display()))?;
    let path = job.join(format!("{latest}.json"));
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))
}

/// The tokens of the last `usage` object the client printed, as one JSON document or a line
/// of JSON each: input, output and cache tokens summed.
pub fn tokens_in(stdout: &str) -> Option<u64> {
    let whole = serde_json::from_str::<Value>(stdout.trim()).ok();
    let lines = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok());
    let usage = whole
        .into_iter()
        .chain(lines)
        .filter_map(|value| value.get("usage").cloned())
        .rfind(Value::is_object)?;
    let fields = [
        "input_tokens",
        "output_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ];
    Some(
        fields
            .iter()
            .filter_map(|field| usage[*field].as_u64())
            .sum(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_is_named_before_its_command() {
        let agent = Agent::parse("sonnet=claude -p --mcp-config {mcp_config} {prompt}")
            .expect("well formed");
        assert_eq!(agent.name, "sonnet");
        assert_eq!(
            agent.words,
            ["claude", "-p", "--mcp-config", "{mcp_config}", "{prompt}"]
        );
    }

    #[test]
    fn an_agent_with_no_command_is_refused() {
        assert!(Agent::parse("sonnet=").is_err());
        assert!(Agent::parse("claude -p").is_err());
    }

    #[test]
    fn the_tokens_are_read_from_one_document_with_its_cache() {
        let stdout = r#"{"type":"result","usage":{"input_tokens":10,"output_tokens":5,
            "cache_read_input_tokens":100,"cache_creation_input_tokens":20}}"#;
        assert_eq!(tokens_in(stdout), Some(135));
    }

    #[test]
    fn the_tokens_are_the_last_usage_of_a_stream_of_lines() {
        let stdout = "{\"type\":\"thread.started\"}\n\
            {\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":7,\"output_tokens\":3}}\n\
            {\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":40,\"output_tokens\":2}}\n";
        assert_eq!(tokens_in(stdout), Some(42));
    }

    #[test]
    fn a_client_that_prints_text_spent_no_tokens_we_know_of() {
        assert_eq!(tokens_in("Done: the job is ready.\n"), None);
    }

    #[test]
    fn a_command_past_its_time_is_stopped() {
        let dir = std::env::temp_dir().join(format!("agent-lab-timeout-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("the temp dir is writable");
        let mut command = Command::new("sleep");
        command.arg("30");
        let status = run_bounded(
            command,
            &dir.join("out"),
            &dir.join("err"),
            Duration::from_millis(300),
        )
        .expect("sleep starts");
        assert_eq!(status, None, "a stopped command has no exit status");
    }

    #[test]
    fn the_latest_revision_is_the_one_scored() {
        let job = std::env::temp_dir().join(format!("agent-lab-job-{}", std::process::id()));
        let _ = fs::remove_dir_all(&job);
        fs::create_dir_all(&job).expect("the temp dir is writable");
        for (number, status) in [(1, "unclean"), (2, "ok"), (10, "failed")] {
            fs::write(job.join(format!("{number}.toml")), "").expect("writable");
            fs::write(
                job.join(format!("{number}.json")),
                format!(r#"{{"status":"{status}"}}"#),
            )
            .expect("writable");
        }
        let check = latest_check(&job).expect("the job has revisions");
        assert_eq!(check["status"], "failed", "revision 10 is after revision 2");
    }
}
