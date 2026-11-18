//! The open window, when there is one: each revision kept is handed to it to run its recipe on
//! its plate, over the loopback port its file names (`docs/design/window-link.md`).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::TcpStream;

use crate::jobs::Revision;

/// The schema of the line the window reads.
const SCHEMA: u64 = 2;
/// How long the window may take to take the connection.
const CONNECT_WAIT: Duration = Duration::from_secs(1);
/// How long it may take to answer: its next frame, which a busy plate can hold up.
const REPLY_WAIT: Duration = Duration::from_secs(15);

/// How long a window started for `show_in_window` may take to listen.
const START_WAIT: Duration = Duration::from_secs(30);
/// How often its file is read again meanwhile.
const START_POLL: Duration = Duration::from_millis(500);

/// Where the window says it listens, or no window at all, and the programs that start one.
#[derive(Debug, Clone, Default)]
pub struct Window {
    file: Option<PathBuf>,
    apps: Vec<PathBuf>,
}

impl Window {
    /// The window `file` names, started where none answers by the first of `apps` that runs.
    pub fn new(file: Option<PathBuf>, apps: Vec<PathBuf>) -> Self {
        Self { file, apps }
    }

    /// Hands `revision` to the window marked `open`, so it is shown whether the window
    /// follows its agents or not, starting the window first where none answers (ADR 0465).
    pub async fn show_now(&self, revision: &Revision) -> Result<(Value, bool)> {
        if revision.steps.is_none() {
            bail!(
                "revision {} keeps no recipe for the window to run",
                revision.number
            );
        }
        if self.file.is_none() {
            bail!("this server was started with --no-window");
        }
        if let Some(answer) = self.show(revision, true).await {
            return Ok((answer, false));
        }
        self.start()?;
        let started = Instant::now();
        while started.elapsed() < START_WAIT {
            tokio::time::sleep(START_POLL).await;
            if let Some(answer) = self.show(revision, true).await {
                return Ok((answer, true));
            }
        }
        bail!(
            "Encrust was started but did not listen within {} s",
            START_WAIT.as_secs()
        )
    }

    /// Starts the window by the first program that runs, left running on its own.
    fn start(&self) -> Result<()> {
        for app in &self.apps {
            let spawned = Command::new(app)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if spawned.is_ok() {
                return Ok(());
            }
        }
        bail!(
            "no Encrust window is open and none could be started from {}; open Encrust and \
             ask again",
            self.apps
                .iter()
                .map(|app| app.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// Hands `revision` to the window and answers what it said; `None` when no window is
    /// open or the revision has no recipe to run, so the tool's answer says nothing of one.
    /// One not marked `open` is run only while the window follows its agents.
    pub async fn show(&self, revision: &Revision, open: bool) -> Option<Value> {
        let steps = revision.steps.as_ref()?;
        let text = tokio::fs::read_to_string(self.file.as_ref()?).await.ok()?;
        let found: Value = serde_json::from_str(&text).ok()?;
        let port = u16::try_from(found["port"].as_u64()?).ok()?;
        let token = found["token"].as_str()?;
        let connected = tokio::time::timeout(CONNECT_WAIT, TcpStream::connect(("127.0.0.1", port)));
        // A file left by a window that has closed names a port nobody listens on.
        let stream = connected.await.ok()?.ok()?;
        let line = json!({
            "schema": SCHEMA,
            "token": token,
            "job": revision.job,
            "revision": revision.number,
            "model": revision.recipe.model,
            "printer": revision.recipe.printer,
            "resin": revision.recipe.resin,
            "recipe": steps,
            "open": open,
        });
        Some(
            ask(stream, &line)
                .await
                .unwrap_or_else(|error| json!({ "shown": false, "why_not": format!("{error:#}") })),
        )
    }
}

async fn ask(mut stream: TcpStream, line: &Value) -> Result<Value> {
    let mut text = line.to_string();
    text.push('\n');
    stream.write_all(text.as_bytes()).await?;
    let mut reply = String::new();
    let mut reader = BufReader::new(stream);
    tokio::time::timeout(REPLY_WAIT, reader.read_line(&mut reply))
        .await
        .context("the window did not answer in time")??;
    if reply.trim().is_empty() {
        bail!("the window closed the connection");
    }
    serde_json::from_str(&reply).context("the window's answer does not read")
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;
    use crate::recipe::Recipe;

    fn revision() -> Revision {
        Revision {
            job: "cube-1".into(),
            number: 2,
            recipe: Recipe::bare("/m/cube.stl".into(), "p".into(), "r".into()),
            steps: Some("name = \"r\"\nformat = 1\n".into()),
        }
    }

    fn file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "encrust-mcp-window-{name}-{}.json",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn a_revision_with_no_recipe_is_not_handed_over() {
        let unstepped = Revision {
            steps: None,
            ..revision()
        };
        assert_eq!(
            Window::new(Some(file("unstepped")), Vec::new())
                .show(&unstepped, false)
                .await,
            None
        );
    }

    #[tokio::test]
    async fn with_no_window_file_there_is_no_window() {
        let window = Window::new(Some(file("none")), Vec::new());
        assert_eq!(window.show(&revision(), false).await, None);
    }

    #[tokio::test]
    async fn a_revision_reaches_the_window_with_its_token_and_its_answer_comes_back() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a free port");
        let port = listener.local_addr().expect("bound").port();
        let path = file("open");
        std::fs::write(&path, json!({"port": port, "token": "t0"}).to_string())
            .expect("the temp dir is writable");
        let window = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("the server connects");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("a line");
            reader
                .get_mut()
                .write_all(b"{\"shown\":true}\n")
                .await
                .expect("answered");
            serde_json::from_str::<Value>(&line).expect("the line is JSON")
        });
        let answered = Window::new(Some(path), Vec::new())
            .show(&revision(), true)
            .await;
        assert_eq!(answered, Some(json!({"shown": true})));
        let line = window.await.expect("the window's task ends");
        assert_eq!(line["token"], "t0");
        assert_eq!(
            (line["job"].as_str(), line["revision"].as_u64()),
            (Some("cube-1"), Some(2))
        );
        assert_eq!(line["model"], "/m/cube.stl");
        assert_eq!(line["recipe"], "name = \"r\"\nformat = 1\n");
        assert_eq!(line["open"], true);
    }

    #[tokio::test]
    async fn with_no_window_and_nothing_to_start_one_the_client_is_told_to_open_encrust() {
        let window = Window::new(Some(file("nothing")), vec!["/no/such/encrust-app".into()]);
        let error = window
            .show_now(&revision())
            .await
            .expect_err("no window could be shown");
        assert!(format!("{error:#}").contains("open Encrust"), "{error:#}");
    }
}
