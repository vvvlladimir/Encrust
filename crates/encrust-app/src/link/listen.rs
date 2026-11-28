//! The loopback port the window listens on for `encrust-mcp`, and the file that tells the
//! server where it is and the token it must give (`docs/design/window-link.md`).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use agent_link::{SHOW_SCHEMA, ShowReply, WINDOW_FILE, WindowFile};

use super::line::{Line, read as read_line};

/// The longest line read from a connection, bytes: a plate's path and its edits take a few
/// thousand.
const MOST_BYTES: u64 = 1 << 20;
/// How long a connection may take to send its line.
const READ_WAIT: Duration = Duration::from_secs(5);
/// How long a line waits for the window to answer it: edits applied wait for their jobs to
/// land, and the server waits longer, so it is never told no of edits still landing.
const REPLY_WAIT: Duration = Duration::from_mins(10);
/// The most connections answered at once; the server holds one a call, a few calls at a time.
const MOST_OPEN: usize = 8;
/// What a connection past `MOST_OPEN` is told.
const BUSY: &str = "the window is answering as many connections as it takes; ask again";

/// A request from the server, and where its reply goes.
pub type Asked = (Line, Sender<Value>);

/// The port listened on, while its file says so.
#[derive(Debug)]
pub struct Listening {
    file: PathBuf,
    token: String,
    pub asked: Receiver<Asked>,
}

impl Listening {
    /// Listens on a free loopback port, a thread of its own answering each connection's one
    /// line, and writes `file` for the server; `wake` brings a frame round to answer.
    pub fn start(file: PathBuf, wake: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let token = token();
        write_file(&file, port, &token)?;
        let (sender, asked) = mpsc::channel();
        let link = Arc::new(Shared {
            token: token.clone(),
            sender,
            wake: Box::new(wake),
            open: AtomicUsize::new(0),
            gone: AtomicBool::new(false),
        });
        std::thread::Builder::new()
            .name("window-link".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    if link.gone.load(Ordering::Relaxed) {
                        return;
                    }
                    take(stream, &link);
                }
            })?;
        Ok(Self { file, token, asked })
    }
}

/// What every connection's thread shares: the token, the way to the window and the count open.
struct Shared {
    token: String,
    sender: Sender<Asked>,
    wake: Box<dyn Fn() + Send + Sync>,
    open: AtomicUsize,
    /// The window has closed, so nothing is left to listen for.
    gone: AtomicBool,
}

/// Answers `stream` on a thread of its own, so a line waiting on the window holds up no other;
/// past `MOST_OPEN` at once it is refused at once.
fn take(stream: TcpStream, link: &Arc<Shared>) {
    if link.open.fetch_add(1, Ordering::AcqRel) >= MOST_OPEN {
        link.open.fetch_sub(1, Ordering::AcqRel);
        write_reply(&stream, &refused(BUSY.into()));
        return;
    }
    let shared = Arc::clone(link);
    let spawned = std::thread::Builder::new()
        .name("window-link-line".into())
        .spawn(move || {
            if let Err(Gone) = answer(&stream, &shared) {
                shared.gone.store(true, Ordering::Relaxed);
            }
            shared.open.fetch_sub(1, Ordering::AcqRel);
        });
    if spawned.is_err() {
        link.open.fetch_sub(1, Ordering::AcqRel);
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
fn answer(stream: &TcpStream, link: &Shared) -> Result<(), Gone> {
    let mut line = String::new();
    let read = stream
        .set_read_timeout(Some(READ_WAIT))
        .and_then(|()| BufReader::new(stream.take(MOST_BYTES)).read_line(&mut line));
    if read.is_err() {
        return Ok(());
    }
    let reply = match read_line(&line, &link.token) {
        Err(why) => refused(why),
        Ok(line) => {
            let (reply, answered) = mpsc::channel();
            link.sender.send((line, reply)).map_err(|_| Gone)?;
            (link.wake)();
            answered
                .recv_timeout(REPLY_WAIT)
                .unwrap_or_else(|_| refused("the window did not answer in time".into()))
        }
    };
    write_reply(stream, &reply);
    Ok(())
}

fn write_reply(stream: &TcpStream, reply: &Value) {
    let mut out = serde_json::to_string(reply).unwrap_or_default();
    out.push('\n');
    let _ = (&*stream).write_all(out.as_bytes());
}

fn refused(why: String) -> Value {
    serde_json::to_value(ShowReply::refused(why)).unwrap_or_default()
}

/// Where the server looks for an open window, beside its jobs in the user's cache.
pub fn default_file() -> Option<PathBuf> {
    Some(agent_link::cache_dir()?.join(WINDOW_FILE))
}

fn write_file(file: &Path, port: u16, token: &str) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let text = WindowFile {
        schema: SHOW_SCHEMA,
        port,
        token: token.to_owned(),
        pid: std::process::id(),
        version: Some(env!("CARGO_PKG_VERSION").to_owned()),
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    // Renamed into place, so a server never reads half a file; the token is the user's alone.
    let partial = file.with_extension("json.part");
    let mut opened = options
        .open(&partial)
        .with_context(|| format!("writing {}", partial.display()))?;
    opened.write_all(&serde_json::to_vec(&text)?)?;
    drop(opened);
    std::fs::rename(&partial, file).with_context(|| format!("writing {}", file.display()))
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
