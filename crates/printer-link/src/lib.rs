//! A printer on the local network, whichever protocol it speaks: what the window and the
//! command line both call to scan for, send to, start and ask one. See ADR 0182.

mod error;
mod scan;
mod state;
mod wire;

pub use error::SendError;
pub use net_sdcp::Transfer;
pub use scan::{Found, scan};
pub use state::State;
pub use wire::{Wire, start_print, state, upload};
