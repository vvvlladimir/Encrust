//! A resin another program exported: the `.cfgx` JSON or the `key = value` material INI. The
//! first byte picks which, so neither spends the session as the other.
#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use printer_profiles::ForeignResin;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    let name = if selector % 2 == 0 { "fuzz.cfgx" } else { "fuzz.ini" };
    let _ = ForeignResin::from_str(text, Path::new(name));
});
