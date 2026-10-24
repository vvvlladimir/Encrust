//! The engine as a browser runs it: `.encrust` bytes in, sliced-file bytes out, no file system,
//! thread or clock (ADR 0177). `bindings` is what `wasm-bindgen` exports.

mod bindings;
mod slice;

pub use slice::{Profiles, Sliced, WebError, slice_project};
