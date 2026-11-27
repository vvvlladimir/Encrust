//! The project's own test prints, built in code with the mark raised on each, so the window and
//! the command line print the same ones (ADR 0232, 0461).

mod detail;
mod exposure;
mod fit;
mod mark;
mod solid;
mod surface;
mod test_print;

pub use solid::stl;
pub use test_print::TestPrint;
