//! A printer on the local network, whichever protocol it speaks: what the window and the
//! command line both call to scan for, send to, start, ask, watch and control one. See ADR 0182.

mod control;
mod error;
mod scan;
mod state;
mod watch;
mod wire;

pub use control::{Command, camera, control, history, picture};
pub use error::SendError;
pub use net_sdcp::{Ending, Task, Transfer};
pub use scan::{Found, scan};
pub use state::State;
pub use watch::watch;
pub use wire::{Wire, start_print, state, upload};
