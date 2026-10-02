use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufReader, Read as _};
use std::path::Path;
use std::time::Duration;

use md5::{Digest as _, Md5};
use serde::Deserialize;

use std::time::Instant;

use crate::control::{CMD_UPLOAD, Control};
use crate::error::SdcpError;
use crate::message::token;
use crate::printer::{Fetching, Printer, Transport};
use crate::serve::FileServer;

/// The board serves its upload endpoint on the control port, see docs/formats/sdcp.md.
const UPLOAD_PORT: u16 = 3030;

/// The protocol fixes the packet at 1 MB; a board rejects an offset it did not expect.
const CHUNK: usize = 1024 * 1024;

/// A packet is small and the network is local, so a stalled one is a dead one.
const CHUNK_TIMEOUT: Duration = Duration::from_secs(60);

/// Separates the form fields; any string the file cannot contain will do.
const BOUNDARY: &str = "----EncrustSdcpBoundary7Ld4Kq";

/// How much of the file has reached the printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub sent_bytes: u64,
    pub total_bytes: u64,
}

impl Transfer {
    pub fn fraction(self) -> f32 {
        if self.total_bytes == 0 {
            return 1.0;
        }
        self.sent_bytes as f32 / self.total_bytes as f32
    }
}

/// Sends a sliced file to a printer and returns the name it landed under.
///
/// `progress` is called as the file moves and stops the transfer by returning `false`.
/// Which way the file travels is the board's to decide: a version 3 board is posted to, and
/// the generation before it is told where to fetch from. See `docs/formats/sdcp.md`.
pub fn upload(
    printer: &Printer,
    path: &Path,
    progress: &mut dyn FnMut(Transfer) -> bool,
) -> Result<String, SdcpError> {
    match printer.transport {
        Transport::WebSocket => push(printer, path, progress),
        Transport::Mqtt => pull(printer, path, progress),
    }
}

/// How long the board is given to connect to our broker and subscribe.
const INVITE_TIMEOUT: Duration = Duration::from_secs(10);

/// A board reports every `REPORT_PERIOD_MS` while it fetches; this is several of those, so
/// a silent board is a stalled one rather than a slow one.
const REPORT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long one look at the connection waits. Short, because what has arrived is only
/// acted on between looks: the board's last report is what says the transfer is over.
const WATCH_POLL: Duration = Duration::from_millis(500);

/// Tells a version 1 board to fetch the file from a server this opens for the transfer.
///
/// The URL carries the literal `${ipaddr}`: the board substitutes the address it is
/// connected to, which saves this having to work out which of our addresses it can see.
fn pull(
    printer: &Printer,
    path: &Path,
    progress: &mut dyn FnMut(Transfer) -> bool,
) -> Result<String, SdcpError> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let total_bytes = std::fs::metadata(path)?.len();
    let digest = md5_of(path)?;

    // A route nobody can guess, so a board holding an old URL cannot fetch this one.
    let route = format!("/{}.{extension}", token());
    let server = FileServer::serve(path, &route)?;
    let mut control = Control::connect(printer, INVITE_TIMEOUT)?;
    control.request(
        CMD_UPLOAD,
        Fetch {
            check: 0,
            clean_cache: 1,
            compress: 0,
            file_size: total_bytes,
            filename: &filename,
            md5: &digest,
            url: &format!("http://${{ipaddr}}:{}{route}", server.port()),
        },
    )?;

    watch(&mut control, total_bytes, progress)?;
    Ok(filename)
}

/// Follows the board's reports until it has the file, gave up, or went quiet.
fn watch(
    control: &mut Control,
    total_bytes: u64,
    progress: &mut dyn FnMut(Transfer) -> bool,
) -> Result<(), SdcpError> {
    let mut last = Instant::now();
    let mut seen = 0;
    loop {
        control.pump(None, Instant::now() + WATCH_POLL)?;
        let transfer = control.status().file_transfer_info.clone();
        match transfer.state() {
            Fetching::Done => {
                progress(Transfer {
                    sent_bytes: total_bytes,
                    total_bytes,
                });
                return Ok(());
            }
            Fetching::Failed => {
                return Err(SdcpError::FetchFailed {
                    filename: transfer.filename,
                });
            }
            Fetching::Running => {}
        }
        if transfer.download_offset != seen {
            seen = transfer.download_offset;
            last = Instant::now();
        }
        if !progress(Transfer {
            sent_bytes: seen.min(total_bytes),
            total_bytes,
        }) {
            return Err(SdcpError::Cancelled);
        }
        if last.elapsed() >= REPORT_TIMEOUT {
            return Err(SdcpError::TimedOut {
                what: "the board's report while it fetched the file",
                seconds: REPORT_TIMEOUT.as_secs(),
            });
        }
    }
}

/// Posts a file to a version 3 board a megabyte at a time.
fn push(
    printer: &Printer,
    path: &Path,
    progress: &mut dyn FnMut(Transfer) -> bool,
) -> Result<String, SdcpError> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let total_bytes = std::fs::metadata(path)?.len();
    let digest = md5_of(path)?;
    let uuid = token();
    let url = format!("http://{}:{UPLOAD_PORT}/uploadFile/upload", printer.address);

    let agent = ureq::config::Config::builder()
        .timeout_global(Some(CHUNK_TIMEOUT))
        .build()
        .new_agent();
    let mut reader = BufReader::new(File::open(path)?);
    let mut packet = vec![0u8; CHUNK];
    let mut sent_bytes = 0;
    while sent_bytes < total_bytes {
        let read = read_packet(&mut reader, &mut packet)?;
        let form = Form {
            digest: &digest,
            offset: sent_bytes,
            uuid: &uuid,
            total_bytes,
            filename: &filename,
        };
        post(&agent, &url, &form, &packet[..read])?;
        sent_bytes += read as u64;
        if !progress(Transfer {
            sent_bytes,
            total_bytes,
        }) {
            return Err(SdcpError::Cancelled);
        }
    }
    Ok(filename)
}

/// A short read is not the end of the file, so the packet is filled before it is sent.
fn read_packet(reader: &mut impl std::io::Read, packet: &mut [u8]) -> Result<usize, SdcpError> {
    let mut filled = 0;
    while filled < packet.len() {
        match reader.read(&mut packet[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}

fn post(agent: &ureq::Agent, url: &str, form: &Form, packet: &[u8]) -> Result<(), SdcpError> {
    let content_type = format!("multipart/form-data; boundary={BOUNDARY}");
    let response = agent
        .post(url)
        .header("Content-Type", content_type)
        .send(&form.body(packet)[..])?;
    let text = response.into_body().read_to_string()?;
    let reply: Reply = serde_json::from_str(&text)?;
    if reply.success {
        return Ok(());
    }
    Err(SdcpError::TransferRefused {
        offset: form.offset,
        reason: reply.reason(),
    })
}

/// What command 256 tells a board to fetch, in the words the protocol uses.
#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct Fetch<'a> {
    check: u32,
    clean_cache: u32,
    compress: u32,
    file_size: u64,
    filename: &'a str,
    #[serde(rename = "MD5")]
    md5: &'a str,
    #[serde(rename = "URL")]
    url: &'a str,
}

/// The fields one packet carries, in the order the protocol lists them.
struct Form<'a> {
    digest: &'a str,
    offset: u64,
    uuid: &'a str,
    total_bytes: u64,
    filename: &'a str,
}

impl Form<'_> {
    fn body(&self, packet: &[u8]) -> Vec<u8> {
        let mut body = Vec::with_capacity(packet.len() + 512);
        let mut field = |name: &str, value: &str| {
            body.extend_from_slice(
                format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
                )
                .as_bytes(),
            );
        };
        field("S-File-MD5", self.digest);
        field("Check", "1");
        field("Offset", &self.offset.to_string());
        field("Uuid", self.uuid);
        field("TotalSize", &self.total_bytes.to_string());
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"File\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                self.filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(packet);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        body
    }
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    code: String,
    #[serde(default)]
    messages: Option<Vec<Complaint>>,
}

impl Reply {
    /// The board reports the offset it disagrees with under a field name of its own.
    fn reason(&self) -> String {
        let listed = self
            .messages
            .iter()
            .flatten()
            .map(|complaint| match &complaint.message {
                serde_json::Value::String(text) => text.clone(),
                other => transfer_error(other.as_i64().unwrap_or_default()).to_string(),
            })
            .collect::<Vec<_>>();
        if listed.is_empty() {
            return format!("error {}", self.code);
        }
        listed.join(", ")
    }
}

#[derive(Deserialize)]
struct Complaint {
    #[serde(default)]
    message: serde_json::Value,
}

fn transfer_error(code: i64) -> &'static str {
    match code {
        -1 => "the offset is not a position in a file",
        -2 => "the offset does not continue the file it is holding",
        -3 => "it could not open the file",
        _ => "it gave no reason",
    }
}

fn md5_of(path: &Path) -> Result<String, SdcpError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Md5::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        match reader.read(&mut buffer)? {
            0 => break,
            read => hasher.update(&buffer[..read]),
        }
    }
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::IpAddr;

    fn a_form() -> Form<'static> {
        Form {
            digest: "d41d8cd98f00b204e9800998ecf8427e",
            offset: 1_048_576,
            uuid: "0123456789abcdef0123456789abcdef",
            total_bytes: 2_097_152,
            filename: "model.goo",
        }
    }

    fn a_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let mut file = File::create(&path).expect("the temporary directory is writable");
        file.write_all(bytes).expect("the bytes are written");
        path
    }

    #[test]
    fn a_packet_carries_every_field_the_protocol_asks_for() {
        let body = a_form().body(b"\x01\x02\x03");
        let text = String::from_utf8_lossy(&body);
        for field in ["S-File-MD5", "Check", "Offset", "Uuid", "TotalSize", "File"] {
            assert!(
                text.contains(&format!("name=\"{field}\"")),
                "{field} is missing"
            );
        }
        assert!(text.contains("filename=\"model.goo\""));
        assert!(text.ends_with(&format!("--{BOUNDARY}--\r\n")));
        assert!(text.contains("1048576"), "the offset is sent as written");
    }

    #[test]
    fn the_digest_is_the_md5_of_the_whole_file() {
        let path = a_file("encrust-sdcp-digest.bin", b"abc");
        assert_eq!(
            md5_of(&path).expect("the file is readable"),
            "900150983cd24fb0d6963f7d28e17f72",
            "RFC 1321 gives this digest for \"abc\""
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn a_missing_file_is_an_io_error_not_an_empty_digest() {
        let missing = std::env::temp_dir().join("encrust-sdcp-not-here.bin");
        assert!(matches!(md5_of(&missing), Err(SdcpError::Io(_))));
    }

    #[test]
    fn a_refusal_names_the_offset_the_board_disagrees_with() {
        let reply: Reply = serde_json::from_str(
            r#"{"code":"111111","success":false,
                "messages":[{"field":"common_field","message":-2}]}"#,
        )
        .expect("the reply is well formed");
        assert!(reply.reason().contains("does not continue"));
    }

    #[test]
    fn progress_returning_false_cancels_the_transfer() {
        let path = a_file("encrust-sdcp-cancel.bin", b"0123456789");
        let printer = Printer {
            name: "Saturn".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([127, 0, 0, 1]),
            mainboard_id: "ff".into(),
            brand_id: "00".into(),
            firmware: String::new(),
            protocol: String::new(),
            transport: crate::printer::Transport::WebSocket,
        };
        let error = upload(&printer, &path, &mut |_| false).expect_err("nothing is listening");
        assert!(matches!(
            error,
            SdcpError::Transport(_) | SdcpError::Cancelled
        ));
        std::fs::remove_file(path).ok();
    }
}
