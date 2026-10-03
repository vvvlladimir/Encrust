//! STL, OBJ and 3MF, and the textures a file can carry with it.
#![no_main]

use std::io::Cursor;
use std::path::Path;

use core_mesh_io::{ModelFile, loader_for_extension};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The first byte picks the loader so one target reaches all three, and the rest is the file.
    let Some((selector, body)) = data.split_first() else {
        return;
    };
    let extension = match selector % 3 {
        0 => "stl",
        1 => "obj",
        _ => "3mf",
    };

    let loader = loader_for_extension(extension).expect("the three loaders are built in");
    let file = ModelFile {
        path: Path::new(extension),
        source: &mut Cursor::new(body),
        beside: &|_| None,
    };
    if let Ok(loaded) = loader.read(file) {
        // A texture is decoded lazily, so loading alone never reaches the image decoders.
        for texture in &loaded.textures {
            let _ = texture.decode();
        }
    }
});
