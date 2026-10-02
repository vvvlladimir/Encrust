use std::hash::{BuildHasher as _, RandomState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::printer::{Attributes, Printer, Status, Transport};

/// Marks the sender of a command as the client rather than the printer's own panel.
const FROM_CLIENT: u32 = 0;

/// One command on its way to a printer, wrapped in the envelope the board expects.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Request<T: Serialize> {
    #[serde(rename = "Id")]
    id: String,
    data: Body<T>,
    /// Version 3 carries the topic in the body. Version 1 does not: the topic it was
    /// published to is the topic, and a field the board does not expect is not added.
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Body<T: Serialize> {
    cmd: u32,
    data: T,
    #[serde(rename = "RequestID")]
    request_id: String,
    #[serde(rename = "MainboardID")]
    mainboard_id: String,
    time_stamp: u64,
    from: u32,
}

impl<T: Serialize> Request<T> {
    /// Addresses a command at one printer, returning it beside the ID of its answer.
    pub(crate) fn new(printer: &Printer, cmd: u32, data: T) -> (Self, String) {
        let request_id = token();
        let topic = match printer.transport {
            Transport::WebSocket => Some(format!("sdcp/request/{}", printer.mainboard_id)),
            Transport::Mqtt => None,
        };
        let request = Self {
            id: printer.brand_id.clone(),
            data: Body {
                cmd,
                data,
                request_id: request_id.clone(),
                mainboard_id: printer.mainboard_id.clone(),
                time_stamp: seconds_now(),
                from: FROM_CLIENT,
            },
            topic,
        };
        (request, request_id)
    }
}

/// The topic a command is published to over MQTT, which unlike the version 3 spelling of
/// the same topic carries a leading slash.
pub(crate) fn request_topic(printer: &Printer) -> String {
    format!("/sdcp/request/{}", printer.mainboard_id)
}

/// Anything a printer sends us, sorted by the topic it arrived on.
#[derive(Debug)]
pub(crate) enum Incoming {
    Response {
        request_id: String,
        ack: Option<u32>,
    },
    Status(Box<Status>),
    Attributes(Box<Attributes>),
    Error(String),
    Other,
}

impl Incoming {
    /// Reads one report. `topic` is the topic it arrived on when the transport knows it,
    /// which is the MQTT case; over a WebSocket the body carries its own.
    pub(crate) fn parse(text: &str, topic: Option<&str>) -> Result<Self, serde_json::Error> {
        let envelope: Envelope = serde_json::from_str(text)?;
        let topic = topic.unwrap_or(envelope.topic.as_str());
        // Version 3 puts the report beside the topic; version 1 puts it inside `Data`.
        let body = envelope.data;
        Ok(match () {
            () if topic.contains("/status/") => {
                match envelope
                    .status
                    .or_else(|| body.as_ref().and_then(|body| body.status.clone()))
                {
                    Some(status) => Self::Status(Box::new(status)),
                    None => Self::Other,
                }
            }
            () if topic.contains("/attributes/") => {
                match envelope
                    .attributes
                    .or_else(|| body.as_ref().and_then(|body| body.attributes.clone()))
                {
                    Some(attributes) => Self::Attributes(Box::new(attributes)),
                    None => Self::Other,
                }
            }
            () if topic.contains("/response/") => body.map_or(Self::Other, |body| Self::Response {
                request_id: body.request_id.unwrap_or_default(),
                ack: body.data.and_then(|inner| inner.ack),
            }),
            () if topic.contains("/error/") => body
                .and_then(|body| body.data)
                .and_then(|inner| inner.error_code)
                .map_or(Self::Other, Self::Error),
            () => Self::Other,
        })
    }
}

/// The topics share one shape, so one struct reads them all and the topic sorts.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Envelope {
    #[serde(default)]
    topic: String,
    #[serde(default)]
    status: Option<Status>,
    #[serde(default)]
    attributes: Option<Attributes>,
    #[serde(default)]
    data: Option<ResponseBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ResponseBody {
    #[serde(default, rename = "RequestID")]
    request_id: Option<String>,
    #[serde(default)]
    data: Option<Payload>,
    /// Where version 1 puts an unprompted report.
    #[serde(default)]
    status: Option<Status>,
    #[serde(default)]
    attributes: Option<Attributes>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Payload {
    #[serde(default)]
    ack: Option<u32>,
    #[serde(default)]
    error_code: Option<String>,
}

fn seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// A request is answered by its ID and a transfer is grouped by one, so two in flight
/// must never collide; the counter separates them inside one second, the hasher
/// separates two runs.
pub(crate) fn token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = RandomState::new();
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let high = state.hash_one(tick);
    let low = state.hash_one(seconds_now() ^ tick);
    format!("{high:016x}{low:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn a_printer() -> Printer {
        Printer {
            name: "Saturn".into(),
            model: "Saturn 4 Ultra".into(),
            brand: "ELEGOO".into(),
            address: IpAddr::from([192, 168, 1, 42]),
            mainboard_id: "000000000001d354".into(),
            brand_id: "6b1c4a2e6b1c4a2e6b1c4a2e6b1c4a2e".into(),
            firmware: "V1.2.3".into(),
            protocol: "V3.0.0".into(),
            transport: Transport::WebSocket,
        }
    }

    fn an_older_printer() -> Printer {
        Printer {
            protocol: "V1.0.0".into(),
            transport: Transport::Mqtt,
            ..a_printer()
        }
    }

    #[test]
    fn a_request_carries_the_board_id_in_its_topic_and_its_body() {
        let (request, id) = Request::new(&a_printer(), 128, serde_json::json!({"StartLayer": 0}));
        let text = serde_json::to_string(&request).expect("the request serialises");
        assert!(text.contains(r#""Topic":"sdcp/request/000000000001d354""#));
        assert!(text.contains(r#""MainboardID":"000000000001d354""#));
        assert!(text.contains(&format!(r#""RequestID":"{id}""#)));
        assert!(text.contains(r#""From":0"#));
    }

    #[test]
    fn a_request_published_over_mqtt_carries_no_topic_field() {
        let (request, _) = Request::new(&an_older_printer(), 0, ());
        let text = serde_json::to_string(&request).expect("the request serialises");
        assert!(!text.contains("Topic"), "got {text}");
        assert_eq!(
            request_topic(&an_older_printer()),
            "/sdcp/request/000000000001d354",
            "the older spelling carries a leading slash"
        );
    }

    #[test]
    fn two_requests_never_share_an_id() {
        let (_, first) = Request::new(&a_printer(), 0, ());
        let (_, second) = Request::new(&a_printer(), 0, ());
        assert_eq!(first.len(), 32, "the protocol asks for a 32-character ID");
        assert_ne!(first, second);
    }

    #[test]
    fn a_response_is_read_as_its_ack() {
        let text = r#"{"Id":"x","Data":{"Cmd":128,"Data":{"Ack":2},
            "RequestID":"abc","MainboardID":"ffffffff","TimeStamp":1},
            "Topic":"sdcp/response/ffffffff"}"#;
        let Incoming::Response { request_id, ack } =
            Incoming::parse(text, None).expect("valid JSON")
        else {
            panic!("the response topic decides");
        };
        assert_eq!(request_id, "abc");
        assert_eq!(ack, Some(2));
    }

    #[test]
    fn a_response_with_no_topic_of_its_own_is_sorted_by_the_one_it_arrived_on() {
        let text = r#"{"Id":"x","Data":{"Cmd":1,"Data":{"Ack":0},
            "RequestID":"130f","MainboardID":"ABCD1234","TimeStamp":8567213}}"#;
        let Incoming::Response { request_id, ack } =
            Incoming::parse(text, Some("/sdcp/response/ABCD1234")).expect("valid JSON")
        else {
            panic!("the topic the message came on decides");
        };
        assert_eq!(request_id, "130f");
        assert_eq!(ack, Some(0));
    }

    #[test]
    fn status_and_attributes_are_told_apart_by_topic() {
        let status = r#"{"Status":{"CurrentStatus":[1],"PrintInfo":{"CurrentLayer":3,
            "TotalLayer":6}},"MainboardID":"ff","Topic":"sdcp/status/ff"}"#;
        let attributes = r#"{"Attributes":{"Name":"Saturn","MachineName":"Saturn 4 Ultra",
            "SupportFileType":["GOO"]},"MainboardID":"ff","Topic":"sdcp/attributes/ff"}"#;
        assert!(matches!(
            Incoming::parse(status, None).expect("valid JSON"),
            Incoming::Status(_)
        ));
        assert!(matches!(
            Incoming::parse(attributes, None).expect("valid JSON"),
            Incoming::Attributes(_)
        ));
    }

    /// Version 1 wraps its unprompted reports in `Data` as well, so the transfer counters
    /// are one level deeper than they are over a WebSocket.
    #[test]
    fn a_version_one_status_is_read_out_of_its_data_block() {
        let text = r#"{"Id":"f252","Data":{"Status":{"CurrentStatus":1,"PreviousStatus":0,
            "PrintInfo":{"Status":0,"CurrentLayer":0,"TotalLayer":0},
            "FileTransferInfo":{"Status":0,"DownloadOffset":1024,"CheckOffset":0,
            "FileTotalSize":4096,"Filename":"model.goo"}},
            "MainboardID":"ABCD1234","TimeStamp":8629636}}"#;
        let Incoming::Status(status) =
            Incoming::parse(text, Some("/sdcp/status/ABCD1234")).expect("valid JSON")
        else {
            panic!("a status message is a status message either way round");
        };
        assert_eq!(status.file_transfer_info.download_offset, 1024);
        assert_eq!(status.file_transfer_info.file_total_size, 4096);
    }

    #[test]
    fn an_error_message_keeps_its_code() {
        let text = r#"{"Id":"x","Data":{"Data":{"ErrorCode":"1"},"MainboardID":"ff"},
            "Topic":"sdcp/error/ff"}"#;
        let Incoming::Error(code) = Incoming::parse(text, None).expect("valid JSON") else {
            panic!("the error topic decides");
        };
        assert_eq!(code, "1");
    }

    #[test]
    fn a_topic_we_do_not_act_on_is_ignored() {
        let text = r#"{"Data":{"Data":{"Message":"hi","Type":1}},"Topic":"sdcp/notice/ff"}"#;
        assert!(matches!(
            Incoming::parse(text, None).expect("valid JSON"),
            Incoming::Other
        ));
    }
}
