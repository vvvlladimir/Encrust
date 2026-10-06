use wasm_bindgen::prelude::*;

use crate::slice_project;

/// A sliced file handed to JavaScript: its bytes and its extension.
#[wasm_bindgen]
pub struct SlicedFile {
    bytes: Vec<u8>,
    extension: String,
}

#[wasm_bindgen]
impl SlicedFile {
    /// The file's bytes, moved out to JavaScript as a `Uint8Array`.
    #[wasm_bindgen(js_name = takeBytes)]
    pub fn take_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }

    #[wasm_bindgen(getter)]
    pub fn extension(&self) -> String {
        self.extension.clone()
    }
}

/// Slices the project in `project`. `created_unix_s` is `Date.now() / 1000`, because the
/// module has no clock.
#[wasm_bindgen]
pub fn slice(project: &[u8], created_unix_s: f64) -> Result<SlicedFile, JsError> {
    let sliced = slice_project(
        project,
        // One thread, so one layer at a time; see ADR 0177.
        1,
        created_unix_s.max(0.0) as u64,
    )
    .map_err(|error| JsError::new(&chain(&error)))?;
    Ok(SlicedFile {
        bytes: sliced.bytes,
        extension: sliced.extension.to_owned(),
    })
}

/// Bytes of linear memory the module holds. Wasm memory only ever grows, so read after a
/// run this is the run's peak.
#[wasm_bindgen(js_name = memoryBytes)]
pub fn memory_bytes() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        (core::arch::wasm32::memory_size(0) * 65_536) as f64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0.0
    }
}

/// The error and every cause under it on one line, since JavaScript sees a message alone.
fn chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}
