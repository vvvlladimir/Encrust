//! Client for SDCP 3.0, the network protocol Elegoo printers speak.
//! Protocol details live in `docs/formats/sdcp.md`.

mod control;
mod discover;
mod error;
mod message;
mod mqtt;
mod printer;
mod serve;
mod upload;

pub use control::Control;
pub use discover::{discover, probe};
pub use error::SdcpError;
pub use printer::{
    Attributes, Fetching, FileTransferInfo, Machine, PrintInfo, Printer, Stage, Status, Transport,
};
pub use upload::{Transfer, upload};
