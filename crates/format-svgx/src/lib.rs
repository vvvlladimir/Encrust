//! The `.svgx` container: a header, two bitmap previews, and an SVG document
//! whose every layer is a group of filled paths in millimetres. Layout is in
//! `docs/formats/svgx.md`.

mod document;
#[cfg(test)]
mod fixtures;
mod header;
mod index;
mod reader;
mod trace;
mod writer;

pub use reader::{OpenSvgx, SvgxReader};
pub use writer::{SvgxSink, SvgxWriter};
