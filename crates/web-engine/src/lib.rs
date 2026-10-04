//! The engine as a browser runs it: the bytes of an `.encrust` project in, the bytes of a
//! sliced file out, with no file system, no thread and no clock of its own (ADR 0177).
//!
//! [`slice_project`] is plain Rust, so the same call runs natively for a measurement;
//! `bindings` is what `wasm-bindgen` exports of it to JavaScript.

mod bindings;
mod slice;

pub use slice::{Sliced, WebError, slice_project};
