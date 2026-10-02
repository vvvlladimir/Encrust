use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// A printer that answered discovery, and the address every later call is made to.
///
/// It serialises so that a client can remember the board a user chose and reach it again
/// without waiting for another broadcast.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Printer {
    /// The name the printer answers to, as set on its own panel.
    pub name: String,
    pub model: String,
    pub brand: String,
    pub address: IpAddr,
    /// Identifies the board across every message; both topics are named after it.
    pub mainboard_id: String,
    /// The brand UUID the printer greeted us with, echoed back in every request.
    pub brand_id: String,
    pub firmware: String,
    pub protocol: String,
    /// How the board is talked to, which is what its protocol version decides.
    pub transport: Transport,
}

/// The two generations of the protocol, which differ in who connects to whom.
///
/// Version 3 boards serve a WebSocket and take the file over HTTP. The generation before
/// them connects out to a broker the client runs and pulls the file from a server the
/// client runs; see `docs/formats/sdcp.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    #[default]
    WebSocket,
    Mqtt,
}

/// What a printer says about itself: what it can do and how big its panel is.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Attributes {
    pub name: String,
    #[serde(rename = "MachineName")]
    pub model: String,
    #[serde(default)]
    pub firmware_version: String,
    /// Panel size as the printer spells it, `7680x4320`.
    #[serde(default)]
    pub resolution: String,
    /// Build volume as the printer spells it, `218x123x230`, in mm.
    #[serde(default, rename = "XYZsize")]
    pub build_volume: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default, rename = "SupportFileType")]
    pub file_types: Vec<String>,
}

impl Attributes {
    /// Whether the printer takes files of this extension, given what it reports.
    pub fn accepts(&self, extension: &str) -> bool {
        self.file_types
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    }

    /// Whether the board carries the file transfer and print control sub-protocols.
    pub fn can_be_sent_to(&self) -> bool {
        let has = |name: &str| self.capabilities.iter().any(|c| c == name);
        has("FILE_TRANSFER") && has("PRINT_CONTROL")
    }
}

/// What a printer is doing right now.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Status {
    /// Several states can be live at once, so the board reports a set.
    #[serde(default, deserialize_with = "one_or_many")]
    pub current_status: Vec<u32>,
    #[serde(default)]
    pub print_info: PrintInfo,
    /// What a board pulling a file has of it. Only version 1 reports this, because only
    /// version 1 pulls.
    #[serde(default)]
    pub file_transfer_info: FileTransferInfo,
}

impl Status {
    /// The state to put in front of the user, the busiest one the board reports.
    pub fn machine(&self) -> Machine {
        self.current_status
            .iter()
            .copied()
            .map(Machine::from_code)
            .max_by_key(|state| u8::from(*state != Machine::Idle))
            .unwrap_or(Machine::Idle)
    }
}

/// The print in progress, kept by the board after it ends.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PrintInfo {
    #[serde(default)]
    pub status: u32,
    #[serde(default)]
    pub current_layer: u32,
    #[serde(default)]
    pub total_layer: u32,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub error_number: u32,
}

impl PrintInfo {
    /// How far through the stack the printer is, or `None` before it starts.
    pub fn fraction(&self) -> Option<f32> {
        (self.total_layer > 0).then(|| self.current_layer as f32 / self.total_layer as f32)
    }

    /// Why the print stopped, or `None` while nothing is wrong.
    pub fn error(&self) -> Option<&'static str> {
        match self.error_number {
            0 => None,
            1 => Some("the file failed its MD5 check"),
            2 => Some("the file could not be read"),
            3 => Some("the masks do not match the panel"),
            4 => Some("the file format was not recognised"),
            5 => Some("the file was sliced for another machine"),
            _ => Some("the printer reported an error it does not name"),
        }
    }
}

/// How far a board has got pulling a file from the server the client runs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FileTransferInfo {
    /// `0` while it runs, `2` once the file is there, `3` when it gave up.
    #[serde(default)]
    pub status: u32,
    /// Bytes fetched so far.
    #[serde(default)]
    pub download_offset: u64,
    #[serde(default)]
    pub file_total_size: u64,
    #[serde(default)]
    pub filename: String,
}

/// What the board says about the file it is pulling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetching {
    Running,
    Done,
    Failed,
}

impl FileTransferInfo {
    /// Whether the transfer is still going, finished or lost.
    pub fn state(&self) -> Fetching {
        match self.status {
            2 => Fetching::Done,
            3 => Fetching::Failed,
            _ => Fetching::Running,
        }
    }
}

/// The top-level machine state, as reported in `CurrentStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    Idle,
    Printing,
    Transferring,
    TestingExposure,
    TestingDevices,
    Unknown(u32),
}

impl Machine {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Idle,
            1 => Self::Printing,
            2 => Self::Transferring,
            3 => Self::TestingExposure,
            4 => Self::TestingDevices,
            other => Self::Unknown(other),
        }
    }

    /// One word for the printer list.
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Printing => "printing",
            Self::Transferring => "receiving",
            Self::TestingExposure => "exposure test",
            Self::TestingDevices => "self-check",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// Firmware in the field sends `CurrentStatus` as a bare number as often as an array.
fn one_or_many<'de, D: serde::Deserializer<'de>>(source: D) -> Result<Vec<u32>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(u32),
        Many(Vec<u32>),
    }
    Ok(match OneOrMany::deserialize(source)? {
        OneOrMany::One(code) => vec![code],
        OneOrMany::Many(codes) => codes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_status_reads_both_a_number_and_an_array() {
        let one: Status = serde_json::from_str(r#"{"CurrentStatus":1}"#).expect("a bare number");
        let many: Status =
            serde_json::from_str(r#"{"CurrentStatus":[0,1]}"#).expect("an array of codes");
        assert_eq!(one.machine(), Machine::Printing);
        assert_eq!(many.machine(), Machine::Printing);
    }

    #[test]
    fn an_empty_status_is_idle() {
        assert_eq!(Status::default().machine(), Machine::Idle);
    }

    #[test]
    fn progress_is_none_until_the_layer_count_is_known() {
        let mut info = PrintInfo::default();
        assert_eq!(info.fraction(), None);
        info.total_layer = 200;
        info.current_layer = 50;
        assert_eq!(info.fraction(), Some(0.25));
    }

    #[test]
    fn a_transfer_is_read_as_running_until_the_board_says_otherwise() {
        let status: Status = serde_json::from_str(
            r#"{"CurrentStatus":1,"FileTransferInfo":{"Status":0,"DownloadOffset":512,
                "FileTotalSize":2048,"Filename":"model.goo"}}"#,
        )
        .expect("the reply is well formed");
        assert_eq!(status.file_transfer_info.state(), Fetching::Running);
        assert_eq!(status.file_transfer_info.download_offset, 512);

        let done = FileTransferInfo {
            status: 2,
            ..FileTransferInfo::default()
        };
        assert_eq!(done.state(), Fetching::Done);
        let failed = FileTransferInfo {
            status: 3,
            ..FileTransferInfo::default()
        };
        assert_eq!(failed.state(), Fetching::Failed);
    }

    #[test]
    fn a_version_three_status_reports_no_transfer_at_all() {
        let status: Status =
            serde_json::from_str(r#"{"CurrentStatus":[0]}"#).expect("the reply is well formed");
        assert_eq!(status.file_transfer_info.file_total_size, 0);
    }

    #[test]
    fn sending_needs_both_sub_protocols() {
        let mut attributes = Attributes {
            capabilities: vec!["FILE_TRANSFER".into()],
            ..Attributes::default()
        };
        assert!(!attributes.can_be_sent_to());
        attributes.capabilities.push("PRINT_CONTROL".into());
        assert!(attributes.can_be_sent_to());
    }
}
