//! A font file off the machine, read into outlines: the face's tables, its `kern` and the
//! curves of every glyph a line of text reaches. The first byte picks the face in a collection.
#![no_main]

use std::sync::Arc;

use core_text::{Font, outline};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&index, rest)) = data.split_first() else {
        return;
    };
    let Ok(font) = Font::new(Arc::new(rest.to_vec()), u32::from(index % 4)) else {
        return;
    };
    let _ = outline(&font, "Encrust 0123 Ag%", 6.0);
});
