//! STL, OBJ and 3MF, and the textures a file can carry with it. The loaders take a path rather
//! than bytes, so the input is written to a temporary file named for the loader this run picked.
#![no_main]

use std::fs;
use std::process;

use core_mesh_io::loader_for_extension;
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

    // The pid keeps parallel `-jobs` runs out of each other's file.
    let path = std::env::temp_dir().join(format!("encrust-fuzz-{}.{extension}", process::id()));
    if fs::write(&path, body).is_err() {
        return;
    }

    let loader = loader_for_extension(extension).expect("the three loaders are built in");
    if let Ok(loaded) = loader.load(&path) {
        // A texture is decoded lazily, so loading alone never reaches the image decoders.
        for texture in &loaded.textures {
            let _ = texture.decode();
        }
    }

    let _ = fs::remove_file(&path);
});
