//! Opening a sliced file to look at, rather than writing one.
//!
//! A file opened this way replaces the plate under the layer slider: what it states comes
//! from the container and nothing from the window's own profiles. See
//! `docs/decisions/0151`.

use crate::files::{self, Handed, Wanted};
use crate::panels::Window;
use crate::preview::Preview;
use crate::status::Status;
use crate::workspace::Mode;

/// The containers the dialog offers, which are the ones a reader claims.
pub const EXTENSIONS: [&str; 13] = [
    "goo", "ctb", "cbddlp", "photon", "pwmx", "pwmo", "pwms", "sl1", "sl1s", "zip", "cxdlp",
    "svgx", "cws",
];

/// Asks for a sliced file and opens it.
pub fn open_dialog(window: &mut Window) {
    if let Some(file) = files::pick(Wanted::SlicedFile) {
        open(
            &mut window.machine.preview,
            window.mode,
            &mut window.machine.status,
            &file,
        );
    }
}

/// Opens `file` and moves to the mode that shows it. The one way a file reaches the slider.
pub fn open(preview: &mut Preview, mode: &mut Mode, status: &mut Status, file: &Handed) {
    match preview.read_file(file) {
        Ok(()) => {
            *mode = Mode::Preview;
            *status = Status::Info(format!("Opened {}", file.path().display()));
        }
        Err(error) => *status = Status::Error(format!("{error:#}")),
    }
}

/// Puts the plate back under the slider. The stack it held is cut again when the mode next
/// refreshes, so nothing has to be remembered here.
pub fn close(preview: &mut Preview, status: &mut Status) {
    if preview.read_path().is_some() {
        preview.close_file();
        *status = Status::Idle;
    }
}
