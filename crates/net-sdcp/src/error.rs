/// Why a printer on the network could not be reached, loaded or started.
#[derive(Debug, thiserror::Error)]
pub enum SdcpError {
    #[error("network call failed")]
    Io(#[from] std::io::Error),

    #[error("the printer sent something this protocol version does not describe")]
    Malformed(#[from] serde_json::Error),

    #[error("the control connection failed")]
    Socket(#[from] Box<tungstenite::Error>),

    #[error("the file transfer failed")]
    Transport(#[from] Box<ureq::Error>),

    #[error("{what} timed out after {seconds} s")]
    TimedOut { what: &'static str, seconds: u64 },

    #[error("the printer refused to start printing: {reason}")]
    PrintRefused { ack: u32, reason: &'static str },

    #[error("the printer rejected the upload at offset {offset}: {reason}")]
    TransferRefused { offset: u64, reason: String },

    #[error("the board gave up fetching {filename}")]
    FetchFailed { filename: String },

    #[error("the board closed the connection")]
    BoardClosed,

    #[error("a board answering as {connected} connected instead of {expected}")]
    WrongBoard { expected: String, connected: String },

    #[error("the board sent {what}")]
    BadPacket { what: &'static str },

    #[error("{printer} prints {supported} files, not .{extension}")]
    UnsupportedFileType {
        printer: String,
        extension: String,
        supported: String,
    },

    #[error("the printer refused to {what}: {reason}")]
    CommandRefused {
        what: &'static str,
        ack: u32,
        reason: &'static str,
    },

    #[error("the printer pointed at {url}, which is not on the printer")]
    ForeignUrl { url: String },

    #[error("cancelled")]
    Cancelled,
}

impl From<tungstenite::Error> for SdcpError {
    fn from(error: tungstenite::Error) -> Self {
        Self::Socket(Box::new(error))
    }
}

impl From<ureq::Error> for SdcpError {
    fn from(error: ureq::Error) -> Self {
        Self::Transport(Box::new(error))
    }
}

impl SdcpError {
    /// Whether the system found no route to the board: it is off, on another network, or
    /// macOS is keeping this program off the local network.
    pub fn is_unreachable(&self) -> bool {
        let io = match self {
            Self::Io(io) => io,
            Self::Socket(socket) => match socket.as_ref() {
                tungstenite::Error::Io(io) => io,
                _ => return false,
            },
            Self::Transport(http) => match http.as_ref() {
                ureq::Error::Io(io) => io,
                _ => return false,
            },
            _ => return false,
        };
        matches!(
            io.kind(),
            std::io::ErrorKind::HostUnreachable | std::io::ErrorKind::NetworkUnreachable
        )
    }
}

/// The `Ack` a board answers a print request with while it is doing something else.
pub(crate) const ACK_BUSY: u32 = 1;

/// What a refused camera request means, by the specification's table for command `386`.
pub(crate) fn video_reason(ack: u32) -> &'static str {
    match ack {
        1 => "it is already streaming to as many viewers as it allows",
        2 => "it has no camera",
        _ => "no reason given",
    }
}

/// The `Ack` a print request comes back with, spelled out for the user.
pub(crate) fn print_ack(ack: u32) -> Result<(), SdcpError> {
    let reason = match ack {
        0 => return Ok(()),
        ACK_BUSY => "it is busy",
        2 => "the file is not there",
        3 => "the file failed its MD5 check",
        4 => "the file could not be read",
        5 => "the masks do not match its panel",
        6 => "it does not recognise the file format",
        7 => "the file was sliced for another machine",
        _ => "no reason given",
    };
    Err(SdcpError::PrintRefused { ack, reason })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_zero_is_the_only_success() {
        assert!(print_ack(0).is_ok());
        for ack in 1..=8 {
            assert!(matches!(
                print_ack(ack),
                Err(SdcpError::PrintRefused { .. })
            ));
        }
    }

    #[test]
    fn no_route_to_the_board_is_told_from_a_board_that_refused() {
        let no_route = std::io::Error::from(std::io::ErrorKind::HostUnreachable);
        assert!(SdcpError::Io(no_route).is_unreachable());
        let wrapped = tungstenite::Error::Io(std::io::ErrorKind::NetworkUnreachable.into());
        assert!(SdcpError::from(wrapped).is_unreachable());
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert!(!SdcpError::Io(refused).is_unreachable());
        assert!(!SdcpError::BoardClosed.is_unreachable());
    }

    #[test]
    fn an_unlisted_ack_still_names_the_code() {
        let Err(SdcpError::PrintRefused { ack, .. }) = print_ack(99) else {
            panic!("an unknown ack is still a refusal");
        };
        assert_eq!(ack, 99);
    }
}
