//! The loopback port the window listens on for `encrust-mcp`, and the file that tells the
//! server where it is and the token it must give (`docs/design/window-link.md`).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::json;

use super::recipe::{Reply, Request, SCHEMA};

/// The longest line read from a connection, bytes: a recipe is a few hundred.
const MOST_BYTES: u64 = 1 << 20;
/// How long a connection may take to send its line.
const READ_WAIT: Duration = Duration::from_secs(5);
/// How long a line waits for the window's frame to answer it.
const REPLY_WAIT: Duration = Duration::from_secs(10);

/// A request from the server, and where its reply goes.
pub type Asked = (Request, Sender<Reply>);

/// The port listened on, while its file says so.
#[derive(Debug)]
pub struct Listening {
    file: PathBuf,
    token: String,
    pub asked: Receiver<Asked>,
}

impl Listening {
    /// Listens on a free loopback port, a thread taking one line a connection, and writes
    /// `file` for the server; `wake` brings a frame round to answer.
    pub fn start(file: PathBuf, wake: impl Fn() + Send + 'static) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let token = token();
        write_file(&file, port, &token)?;
        let (sender, asked) = mpsc::channel();
        let expected = token.clone();
        std::thread::Builder::new()
            .name("window-link".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    // A connection that breaks off is the server's to retry.
                    if let Err(Gone) = answer(&stream, &expected, &sender, &wake) {
                        return;
                    }
                }
            })?;
        Ok(Self { file, token, asked })
    }
}

impl Drop for Listening {
    /// Takes the file away, unless another window has written its own over it since.
    fn drop(&mut self) {
        let ours = std::fs::read_to_string(&self.file).is_ok_and(|text| text.contains(&self.token));
        if ours {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

/// The window has closed, so nothing is left to listen for.
struct Gone;

/// Reads one line, hands it to the window and writes back what the window said.
fn answer(
    stream: &TcpStream,
    token: &str,
    sender: &Sender<Asked>,
    wake: &impl Fn(),
) -> Result<(), Gone> {
    let mut line = String::new();
    let read = stream
        .set_read_timeout(Some(READ_WAIT))
        .and_then(|()| BufReader::new(stream.take(MOST_BYTES)).read_line(&mut line));
    if read.is_err() {
        return Ok(());
    }
    let reply = match serde_json::from_str::<Request>(&line) {
        Err(error) => refused(format!("the line does not read: {error}")),
        Ok(request) if request.token != token => refused("the token is not this window's".into()),
        Ok(request) if request.schema != SCHEMA => {
            refused(format!("schema {} is not {SCHEMA}", request.schema))
        }
        Ok(request) => {
            let (reply, answered) = mpsc::channel();
            sender.send((request, reply)).map_err(|_| Gone)?;
            wake();
            answered
                .recv_timeout(REPLY_WAIT)
                .unwrap_or_else(|_| refused("the window did not answer in time".into()))
        }
    };
    let mut out = serde_json::to_string(&reply).unwrap_or_default();
    out.push('\n');
    let _ = (&*stream).write_all(out.as_bytes());
    Ok(())
}

fn refused(why: String) -> Reply {
    Reply {
        shown: false,
        unshown: Vec::new(),
        why_not: Some(why),
    }
}

/// Where the server looks for an open window, beside its jobs in the user's cache.
pub fn default_file() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Encrust")?;
    Some(dirs.cache_dir().join("window.json"))
}

fn write_file(file: &Path, port: u16, token: &str) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let text = json!({
        "schema": SCHEMA, "port": port, "token": token, "pid": std::process::id(),
    });
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut opened = options
        .open(file)
        .with_context(|| format!("writing {}", file.display()))?;
    opened.write_all(text.to_string().as_bytes())?;
    Ok(())
}

/// 128 bits from the hasher keys the standard library seeds from the system.
fn token() -> String {
    let half = || {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u32(std::process::id());
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(), half())
}
