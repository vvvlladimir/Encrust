//! The open window, when there is one: each revision kept is handed to it to replay on its
//! plate, over the loopback port its file names (`docs/design/window-link.md`).

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::TcpStream;

use crate::jobs::Revision;

/// The schema of the line the window reads.
const SCHEMA: u64 = 1;
/// How long the window may take to take the connection.
const CONNECT_WAIT: Duration = Duration::from_secs(1);
/// How long it may take to answer: its next frame, which a busy plate can hold up.
const REPLY_WAIT: Duration = Duration::from_secs(15);

/// Where the window says it listens, or no window at all.
#[derive(Debug, Clone, Default)]
pub struct Window {
    file: Option<PathBuf>,
}

impl Window {
    pub fn new(file: Option<PathBuf>) -> Self {
        Self { file }
    }

    /// Hands `revision` to the window and answers what it said; `None` when no window is
    /// open, so the tool's answer says nothing of one.
    pub async fn show(&self, revision: &Revision) -> Option<Value> {
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
            "recipe": revision.recipe,
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
        }
    }

    fn file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "encrust-mcp-window-{name}-{}.json",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn with_no_window_file_there_is_no_window() {
        let window = Window::new(Some(file("none")));
        assert_eq!(window.show(&revision()).await, None);
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
        let answered = Window::new(Some(path)).show(&revision()).await;
        assert_eq!(answered, Some(json!({"shown": true})));
        let line = window.await.expect("the window's task ends");
        assert_eq!(line["token"], "t0");
        assert_eq!(
            (line["job"].as_str(), line["revision"].as_u64()),
            (Some("cube-1"), Some(2))
        );
        assert_eq!(line["recipe"]["model"], "/m/cube.stl");
    }
}
