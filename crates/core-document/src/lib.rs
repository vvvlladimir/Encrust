//! The plate as one document with stable object ids, which every front end edits the same
//! way (ADR 0466, `docs/design/document.md`).

mod document;

pub use document::Document;
