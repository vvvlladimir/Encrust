use std::time::Duration;

use serde::Deserialize;

use crate::error::SdcpError;
use crate::printer::Printer;

/// A picture of the plate is a few hundred kilobytes over the local network.
const PICTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest picture taken from a board; a 400 by 300 bitmap is under half a megabyte.
const PICTURE_LIMIT_BYTES: u64 = 8 * 1024 * 1024;

/// One print a board remembers, as command `321` reports it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Task {
    #[serde(default)]
    pub task_id: String,
    /// The file's path on the board's own storage.
    #[serde(default)]
    pub task_name: String,
    /// Where the board serves a picture of the plate, over HTTP.
    #[serde(default)]
    pub thumbnail: String,
    /// Unix seconds.
    #[serde(default)]
    pub begin_time: u64,
    /// Unix seconds; the begin time again for a print stopped before it started.
    #[serde(default)]
    pub end_time: u64,
    #[serde(default)]
    pub task_status: u32,
    #[serde(default)]
    pub already_print_layer: u32,
    #[serde(default)]
    pub slice_information: Option<SliceInformation>,
}

/// What the file said about itself, as far as a list of prints needs it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct SliceInformation {
    #[serde(default)]
    pub total_layer_numbers: u32,
    /// Estimated time of the whole print, in seconds.
    #[serde(default)]
    pub print_time: f64,
    /// Resin the print takes, in ml.
    #[serde(default)]
    pub volume: f64,
}

/// How a remembered print ended, as `TaskStatus` spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Finished,
    Failed,
    Stopped,
    Other,
}

impl Task {
    /// How the print ended, by its `TaskStatus`.
    pub fn ending(&self) -> Ending {
        match self.task_status {
            1 => Ending::Finished,
            2 => Ending::Failed,
            3 => Ending::Stopped,
            _ => Ending::Other,
        }
    }

    /// The file's name without the storage it sits on.
    pub fn file_name(&self) -> &str {
        self.task_name.rsplit('/').next().unwrap_or(&self.task_name)
    }
}

/// Fetches a picture a board named, such as a task's thumbnail, as the bytes it serves.
///
/// Only a URL on the board's own address is fetched: the URL comes off the network, and
/// nothing else on it gets to send this program elsewhere.
pub fn picture(printer: &Printer, url: &str) -> Result<Vec<u8>, SdcpError> {
    if host_of(url) != Some(printer.address.to_string()) {
        return Err(SdcpError::ForeignUrl {
            url: url.to_owned(),
        });
    }
    let agent = ureq::config::Config::builder()
        .timeout_global(Some(PICTURE_TIMEOUT))
        // A redirect would take the fetch off the board the address check let it reach.
        .max_redirects(0)
        .build()
        .new_agent();
    let mut response = agent.get(url).call()?;
    Ok(response
        .body_mut()
        .with_config()
        .limit(PICTURE_LIMIT_BYTES)
        .read_to_vec()?)
}

/// The host of a plain `http://` URL, without its port, read as the request will read it.
fn host_of(url: &str) -> Option<String> {
    let uri: ureq::http::Uri = url.parse().ok()?;
    if uri.scheme_str() != Some("http") {
        return None;
    }
    uri.host().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printer::Transport;
    use std::net::IpAddr;

    /// What a Saturn 4 Ultra on firmware V1.5.2 answered command `321` with, trimmed.
    const TASK: &str = r#"{
        "Thumbnail": "http://192.168.1.133:3030/media/mmcblk0p1/history_image/0e8a.bmp",
        "TaskName": "/media/mmcblk0p3/NEW+Darth+vader.3mf.goo",
        "BeginTime": 1791971587, "EndTime": 1791971587, "TaskStatus": 3,
        "SliceInformation": {"total_layer_numbers": 1710, "print_time": 18885,
            "volume": 235.12747192382812, "machine_name": "ELEGOO Saturn 4 Ultra"},
        "AlreadyPrintLayer": 0, "TaskId": "0e8ac2f2-c7b5-11f1-9191-a4e88d50a1c6",
        "MD5": "b127e08a9a8196f38327b1c2249127f2", "TimeLapseVideoStatus": 0,
        "TimeLapseVideoUrl": "/media/mmcblk0p2/local_6393462.mp4", "ErrorStatusReason": 0
    }"#;

    fn a_board() -> Printer {
        Printer {
            name: "3D Printer".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([192, 168, 1, 133]),
            mainboard_id: "d85f46e4d7d90100".into(),
            brand_id: "00".into(),
            firmware: "V1.5.2".into(),
            protocol: "V3.0.0".into(),
            transport: Transport::WebSocket,
        }
    }

    #[test]
    fn a_stopped_task_names_its_file_and_how_long_it_would_have_taken() {
        let task: Task = serde_json::from_str(TASK).expect("a task from a shipping board");
        assert_eq!(task.ending(), Ending::Stopped, "TaskStatus 3");
        assert_eq!(task.file_name(), "NEW+Darth+vader.3mf.goo");
        let slice = task.slice_information.expect("the board read the file");
        assert_eq!(slice.total_layer_numbers, 1710);
        assert!((slice.print_time - 18885.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_picture_anywhere_but_on_the_board_is_refused() {
        let elsewhere = "http://203.0.113.9:3030/media/thumb.bmp";
        assert!(matches!(
            picture(&a_board(), elsewhere),
            Err(SdcpError::ForeignUrl { .. })
        ));
        let encrypted = "https://192.168.1.133/thumb.bmp";
        assert!(matches!(
            picture(&a_board(), encrypted),
            Err(SdcpError::ForeignUrl { .. })
        ));
        let disguised = "http://192.168.1.133:3030@203.0.113.9/media/thumb.bmp";
        assert!(
            matches!(
                picture(&a_board(), disguised),
                Err(SdcpError::ForeignUrl { .. })
            ),
            "the board's address as a login still sends the request elsewhere"
        );
    }

    #[test]
    fn the_host_is_read_without_its_port_or_path() {
        assert_eq!(
            host_of("http://192.168.1.133:3030/media/a.bmp").as_deref(),
            Some("192.168.1.133")
        );
        assert_eq!(
            host_of("http://192.168.1.133/a.bmp").as_deref(),
            Some("192.168.1.133")
        );
        assert_eq!(host_of("ftp://192.168.1.133/a.bmp"), None);
    }
}
