//! The HTTP server a version 1 board pulls the file from.
//!
//! One file, one route, for as long as the transfer lasts. A board asks for it with a HEAD
//! and then a GET, and may do either more than once, so the route stays up until the
//! transfer ends rather than being served exactly once.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::SdcpError;

/// How long the accept loop waits before it looks at the stop flag again.
const POLL: Duration = Duration::from_millis(100);

/// A request line and its headers are small; anything longer is not one.
const MAX_REQUEST: u64 = 8 * 1024;

/// How much of the file is written to the socket at a time. Peak memory must not follow
/// the stack's size, so the file is streamed rather than read.
const CHUNK: usize = 256 * 1024;

/// A file being served on a port of its own, on a thread of its own.
///
/// Dropping this stops the thread, which is what takes the file off the network again.
pub(crate) struct FileServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FileServer {
    /// Puts `path` up at `route`, on a port the operating system picks.
    pub(crate) fn serve(path: &Path, route: &str) -> Result<Self, SdcpError> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));

        let file = File {
            path: path.to_path_buf(),
            route: route.to_owned(),
            size: std::fs::metadata(path)?.len(),
        };
        let worker_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || run(&listener, &file, &worker_stop));

        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What is being served, and where.
struct File {
    path: PathBuf,
    route: String,
    size: u64,
}

/// Accepts until the transfer is over.
///
/// Nothing an accept can fail with ends the loop: a signal interrupts one under load, and a
/// connection can be aborted before it is taken, and neither means the board has finished
/// fetching. Only the stop flag closes the server.
fn run(listener: &TcpListener, file: &File, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, from)) => {
                if let Err(error) = answer(stream, file) {
                    tracing::debug!(%from, %error, "a request for the file failed");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => {
                if error.kind() != std::io::ErrorKind::WouldBlock {
                    tracing::debug!(%error, "an incoming connection was not taken");
                }
                std::thread::sleep(POLL);
            }
        }
    }
}

fn answer(mut stream: TcpStream, file: &File) -> Result<(), SdcpError> {
    // A socket accepted from a non-blocking listener inherits that flag on BSD, and a read
    // on it then fails the moment the request has not all arrived. That failure closes the
    // connection on an unread request, which resets it and throws the answer away.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;
    let Some((method, target)) = request(&stream)? else {
        return Ok(());
    };

    if target != file.route {
        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
        return done(&stream);
    }
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\
         Content-Length: {}\r\nAccept-Ranges: none\r\nConnection: close\r\n\r\n",
        file.size
    );
    stream.write_all(header.as_bytes())?;
    if method != "HEAD" {
        stream_file(&mut stream, &file.path)?;
    }
    done(&stream)
}

/// The method and target of one request, reading every header before answering.
///
/// The whole request has to be read, not just its first line: closing a socket that still
/// holds unread bytes sends a reset, and a reset throws away the answer already written to
/// it. That is how the board loses a file it had almost fetched.
fn request(stream: &TcpStream) -> Result<Option<(String, String)>, SdcpError> {
    let mut reader = BufReader::new(stream.take(MAX_REQUEST));
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let start = line.clone();
    while !matches!(line.as_str(), "\r\n" | "\n") {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
    }

    let mut parts = start.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Ok(None);
    };
    Ok(Some((method.to_owned(), target.to_owned())))
}

/// Ends one exchange: the writing half is closed so the client sees the end of the body,
/// and the socket is only dropped once it holds nothing unread.
fn done(stream: &TcpStream) -> Result<(), SdcpError> {
    stream.shutdown(Shutdown::Write)?;
    let mut leftover = [0u8; 1024];
    while let Ok(read) = (&*stream).read(&mut leftover) {
        if read == 0 {
            break;
        }
    }
    Ok(())
}

fn stream_file(stream: &mut TcpStream, path: &Path) -> Result<(), SdcpError> {
    let mut file = BufReader::new(std::fs::File::open(path)?);
    let mut chunk = vec![0u8; CHUNK];
    loop {
        match file.read(&mut chunk)? {
            0 => return Ok(stream.flush()?),
            read => stream.write_all(&chunk[..read])?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_file(name: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        let mut file = std::fs::File::create(&path).expect("the temporary directory is writable");
        file.write_all(bytes).expect("the bytes are written");
        path
    }

    /// Sends one request and reads everything that comes back.
    fn ask(port: u16, request: &str) -> Vec<u8> {
        let mut stream =
            TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("the server is on this port");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("the socket takes a timeout");
        stream
            .write_all(request.as_bytes())
            .expect("the server is reading");
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).expect("the server answers");
        answer
    }

    #[test]
    fn the_file_comes_back_whole_under_its_own_route() {
        let path = a_file("encrust-sdcp-serve.goo", b"0123456789");
        let server = FileServer::serve(&path, "/abc.goo").expect("a local port is free");
        let answer = ask(server.port(), "GET /abc.goo HTTP/1.1\r\nHost: x\r\n\r\n");
        let text = String::from_utf8_lossy(&answer);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "got {text}");
        assert!(text.contains("Content-Length: 10"), "got {text}");
        assert!(text.ends_with("0123456789"), "got {text}");
        drop(server);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn a_head_answers_the_size_and_sends_no_body() {
        let path = a_file("encrust-sdcp-serve-head.goo", b"0123456789");
        let server = FileServer::serve(&path, "/abc.goo").expect("a local port is free");
        let answer = ask(server.port(), "HEAD /abc.goo HTTP/1.1\r\nHost: x\r\n\r\n");
        let text = String::from_utf8_lossy(&answer);
        assert!(text.contains("Content-Length: 10"), "got {text}");
        assert!(
            text.ends_with("\r\n\r\n"),
            "a HEAD carries no body: got {text}"
        );
        drop(server);
        std::fs::remove_file(path).ok();
    }

    /// The failure this guards against is a reset: the server closing on unread headers
    /// throws away the body it has already written, and the board fetches nothing.
    #[test]
    fn a_request_with_headers_after_its_first_line_is_still_answered_whole() {
        let path = a_file("encrust-sdcp-serve-headers.goo", b"0123456789");
        let server = FileServer::serve(&path, "/abc.goo").expect("a local port is free");
        let answer = ask(
            server.port(),
            "GET /abc.goo HTTP/1.1\r\nHost: printer\r\nUser-Agent: board\r\n\
             Accept: */*\r\nConnection: close\r\n\r\n",
        );
        assert!(
            String::from_utf8_lossy(&answer).ends_with("0123456789"),
            "the whole body arrives with the headers read"
        );
        drop(server);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn the_route_is_asked_for_by_name_and_nothing_else_is_served() {
        let path = a_file("encrust-sdcp-serve-404.goo", b"secret");
        let server = FileServer::serve(&path, "/abc.goo").expect("a local port is free");
        let answer = ask(server.port(), "GET /other.goo HTTP/1.1\r\nHost: x\r\n\r\n");
        let text = String::from_utf8_lossy(&answer);
        assert!(text.starts_with("HTTP/1.1 404"), "got {text}");
        assert!(!text.contains("secret"));
        drop(server);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn the_same_file_can_be_asked_for_twice() {
        let path = a_file("encrust-sdcp-serve-twice.goo", b"abc");
        let server = FileServer::serve(&path, "/x.goo").expect("a local port is free");
        for _ in 0..2 {
            let answer = ask(server.port(), "GET /x.goo HTTP/1.1\r\nHost: x\r\n\r\n");
            assert!(String::from_utf8_lossy(&answer).ends_with("abc"));
        }
        drop(server);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn dropping_the_server_takes_the_file_off_the_network() {
        let path = a_file("encrust-sdcp-serve-drop.goo", b"abc");
        let server = FileServer::serve(&path, "/x.goo").expect("a local port is free");
        let port = server.port();
        drop(server);
        assert!(
            TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err(),
            "the port is closed once the transfer is over"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn a_file_that_is_not_there_is_refused_before_a_port_is_taken() {
        let missing = std::env::temp_dir().join("encrust-sdcp-not-serving.goo");
        assert!(matches!(
            FileServer::serve(&missing, "/x.goo"),
            Err(SdcpError::Io(_))
        ));
    }
}
