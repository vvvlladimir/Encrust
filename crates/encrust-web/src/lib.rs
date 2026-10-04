//! The Encrust window as a browser runs it: `www/index.html` hands `start` a canvas, and
//! everything else is `encrust-app`. How it is built and served: `docs/design/web-build.md`.

#[cfg(target_arch = "wasm32")]
mod start;

#[cfg(target_arch = "wasm32")]
pub use start::start;
