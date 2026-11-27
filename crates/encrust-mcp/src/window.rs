//! The open window, when there is one: each revision kept is handed to it as the plate its
//! check wrote, over the loopback port its file names (`docs/design/window-link.md`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use agent_link::{
    ApplyReply, HandedPlate, Header, PLATE_SCHEMA, PlateLine, SHOW_SCHEMA, Show, Signed, WindowFile,
};
use anyhow::{Context, Result, bail};
use core_edit::Edit;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::TcpStream;

use crate::jobs::Revision;

/// How long the window may take to take the connection.
const CONNECT_WAIT: Duration = Duration::from_secs(1);
/// How long it may take to answer: its next frame, which a busy plate can hold up.
const REPLY_WAIT: Duration = Duration::from_secs(15);
/// How long edits handed over may take to land, each a tool's job: longer than the window
/// waits for them, so its answer always arrives.
const APPLY_WAIT: Duration = Duration::from_mins(11);

/// How long a window started for `show_in_window` may take to listen.
const START_WAIT: Duration = Duration::from_secs(30);
/// How often its file is read again meanwhile.
const START_POLL: Duration = Duration::from_millis(500);

/// Where the window says it listens, or no window at all, and the programs that start one.
#[derive(Debug, Clone, Default)]
pub struct Window {
    file: Option<PathBuf>,
    apps: Vec<PathBuf>,
    /// Whether the model was told that the window is of another version, once a run.
    told_version: Arc<AtomicBool>,
}

impl Window {
    /// The window `file` names, started where none answers by the first of `apps` that runs.
    pub fn new(file: Option<PathBuf>, apps: Vec<PathBuf>) -> Self {
        Self {
            file,
            apps,
            told_version: Arc::default(),
        }
    }

    /// Hands `revision` to the window marked `open`, so it is shown whether the window
    /// follows its agents or not, starting the window first where none answers (ADR 0462).
    pub async fn show_now(&self, revision: &Revision, plate: &Path) -> Result<(Value, bool)> {
        if self.file.is_none() {
            bail!("this server was started with --no-window");
        }
        if let Some(answer) = self.show(revision, plate, true).await {
            return Ok((answer, false));
        }
        self.start()?;
        let started = Instant::now();
        while started.elapsed() < START_WAIT {
            tokio::time::sleep(START_POLL).await;
            if let Some(answer) = self.show(revision, plate, true).await {
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

    /// Hands `revision` and the `plate` its check wrote to the window, answering what it said or
    /// `None` with no window open; one not `open` shows only while the window follows agents.
    pub async fn show(&self, revision: &Revision, plate: &Path, open: bool) -> Option<Value> {
        let (stream, token) = self.connect().await?;
        let show = Show {
            job: revision.job.clone(),
            revision: revision.number,
            model: revision.recipe.model.clone(),
            printer: revision.recipe.printer.clone(),
            resin: revision.recipe.resin.clone(),
            plate: plate.to_owned(),
            open,
        };
        let line = signed(SHOW_SCHEMA, token, &show)?;
        Some(
            ask(stream, &line, REPLY_WAIT)
                .await
                .unwrap_or_else(|error| json!({ "shown": false, "why_not": format!("{error:#}") })),
        )
    }

    /// The open window's plate as `job` builds on it: its file, version, the ids on it and the
    /// job's model there, or `None` with no window that hands it over (ADR 0456).
    pub async fn plate(&self, job: &str) -> Option<HandedPlate> {
        let (stream, token) = self.connect().await?;
        let line = signed(PLATE_SCHEMA, token, &PlateLine::plate_of(job))?;
        let answer = ask(stream, &line, REPLY_WAIT).await.ok()?;
        serde_json::from_value(answer).ok()
    }

    /// Hands `edits` built on the plate at `base_version` to the window, and its answer.
    pub async fn apply(&self, job: &str, edits: &[Edit], base_version: u64) -> Option<ApplyReply> {
        let (stream, token) = self.connect().await?;
        let line = signed(
            PLATE_SCHEMA,
            token,
            &PlateLine::apply(job, edits, base_version),
        )?;
        let answer = ask(stream, &line, APPLY_WAIT).await.and_then(|answer| {
            serde_json::from_value(answer).context("the window's answer does not read")
        });
        Some(answer.unwrap_or_else(|error| ApplyReply::refused(format!("{error:#}"))))
    }

    /// What to tell the model, once a run, when the open window is of another version than
    /// this server: the user brings the extension to the window's from its File menu.
    pub async fn version_note(&self) -> Option<String> {
        let window = self.read().await?.version?;
        let ours = env!("CARGO_PKG_VERSION");
        if window == ours || self.told_version.swap(true, Ordering::Relaxed) {
            return None;
        }
        Some(format!(
            "Note for the user: this Encrust extension is version {ours} and the Encrust \
             window {window}. To match them, choose File → Use with an AI chat client in the \
             window, which installs the extension of its version."
        ))
    }

    async fn read(&self) -> Option<WindowFile> {
        let text = tokio::fs::read_to_string(self.file.as_ref()?).await.ok()?;
        serde_json::from_str(&text).ok()
    }

    /// A connection to the window its file names, and the token it asks for.
    async fn connect(&self) -> Option<(TcpStream, String)> {
        let WindowFile { port, token, .. } = self.read().await?;
        let connected = tokio::time::timeout(CONNECT_WAIT, TcpStream::connect(("127.0.0.1", port)));
        // A file left by a window that has closed names a port nobody listens on.
        let stream = connected.await.ok()?.ok()?;
        Some((stream, token))
    }
}

/// `body` under the header of `schema` and `token`, as a line to send.
fn signed(schema: u64, token: String, body: &impl Serialize) -> Option<Value> {
    let header = Header { schema, token };
    serde_json::to_value(Signed { header, body }).ok()
}

async fn ask(mut stream: TcpStream, line: &Value, wait: Duration) -> Result<Value> {
    let mut text = line.to_string();
    text.push('\n');
    stream.write_all(text.as_bytes()).await?;
    let mut reply = String::new();
    let mut reader = BufReader::new(stream);
    tokio::time::timeout(wait, reader.read_line(&mut reply))
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

    const PLATE: &str = "/jobs/cube-1/2.encrust";

    fn file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "encrust-mcp-window-{name}-{}.json",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn with_no_window_file_there_is_no_window() {
        let window = Window::new(Some(file("none")), Vec::new());
        assert_eq!(
            window.show(&revision(), Path::new(PLATE), false).await,
            None
        );
    }

    #[tokio::test]
    async fn a_revision_reaches_the_window_with_its_token_and_its_answer_comes_back() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a free port");
        let port = listener.local_addr().expect("bound").port();
        let path = file("open");
        let found = json!({"schema": 4, "port": port, "token": "t0", "pid": 1});
        std::fs::write(&path, found.to_string()).expect("the temp dir is writable");
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
            .show(&revision(), Path::new(PLATE), true)
            .await;
        assert_eq!(answered, Some(json!({"shown": true})));
        let line = window.await.expect("the window's task ends");
        assert_eq!(line["token"], "t0");
        assert_eq!(
            (line["job"].as_str(), line["revision"].as_u64()),
            (Some("cube-1"), Some(2))
        );
        assert_eq!(line["model"], "/m/cube.stl");
        assert_eq!(line["plate"], PLATE);
        assert_eq!(line["schema"], 4);
        assert_eq!(line["open"], true);
    }

    #[tokio::test]
    async fn with_no_window_and_nothing_to_start_one_the_client_is_told_to_open_encrust() {
        let window = Window::new(Some(file("nothing")), vec!["/no/such/encrust-app".into()]);
        let error = window
            .show_now(&revision(), Path::new(PLATE))
            .await
            .expect_err("no window could be shown");
        assert!(format!("{error:#}").contains("open Encrust"), "{error:#}");
    }

    /// A window whose file says it is of `version`.
    fn window_of(version: &str, name: &str) -> Window {
        let path = file(name);
        let said = json!({ "schema": 4, "port": 1, "token": "t", "pid": 1, "version": version });
        std::fs::write(&path, said.to_string()).expect("the temp dir is writable");
        Window::new(Some(path), Vec::new())
    }

    #[tokio::test]
    async fn a_window_of_another_version_is_told_once() {
        let window = window_of("0.0.1", "other-version");
        let note = window.version_note().await.expect("a note the first time");
        assert!(note.contains("0.0.1"), "{note}");
        assert_eq!(window.version_note().await, None, "told once a run");
    }

    #[tokio::test]
    async fn a_window_of_this_version_is_not_mentioned() {
        let window = window_of(env!("CARGO_PKG_VERSION"), "same-version");
        assert_eq!(window.version_note().await, None);
    }
}
