//! What both revisions of the container share: the magic they are headed by, the model
//! code their headers name, and how a block is built before it is summed.

use std::io::{self, Cursor};

use core_format::{Fields, FormatError};

/// What every file of this family is headed by, nul included.
pub(crate) const MAGIC: &[u8] = b"CXSW3DV2\0";

/// Which revision of the container to write.
///
/// Both carry the same magic and differ in everything behind it: version 3 holds a layer
/// as vertical lines, version 4 as the Chitu family's runs. See
/// `docs/formats/creality.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CxdlpVersion {
    #[default]
    V3,
    V4,
}

/// The `CL` or `CT` code the header must carry, picked out of `name`.
///
/// The firmware matches this and nothing else, and a profile states the model the way a
/// user reads it — `Halot One CL-60`. A name carrying no code is refused rather than
/// guessed at, because a machine that does not recognise its own model does not print,
/// which is also what tells a front end not to offer this container at all.
pub fn model_code(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    for start in 0..bytes.len().saturating_sub(2) {
        let prefix = &name[start..start + 2];
        if !matches!(prefix, "CL" | "CT") {
            continue;
        }
        let rest = &name[start + 2..];
        let rest = rest.strip_prefix('-').unwrap_or(rest);
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            continue;
        }
        let suffix: String = rest[digits.len()..]
            .chars()
            .take(1)
            .filter(char::is_ascii_alphabetic)
            .collect();
        return Some(format!("{prefix}-{digits}{suffix}"));
    }
    None
}

/// One block of the file, built in memory so it can be summed as well as written.
pub(crate) fn block(
    write: impl FnOnce(&mut Fields<'_>) -> io::Result<()>,
) -> Result<Vec<u8>, FormatError> {
    block_at(0, write).map(|(bytes, ())| bytes)
}

/// The same, for a block whose own fields carry the absolute address of what follows
/// them: the cursor stands at `at`, which is where the block lands in the file, and the
/// bytes in front of it are dropped.
pub(crate) fn block_at<T>(
    at: u64,
    write: impl FnOnce(&mut Fields<'_>) -> io::Result<T>,
) -> Result<(Vec<u8>, T), FormatError> {
    let mut buffer = Cursor::new(Vec::new());
    let value = {
        let mut fields = Fields::new(&mut buffer);
        fields.seek_to(at)?;
        write(&mut fields)?
    };
    let mut bytes = buffer.into_inner();
    Ok((bytes.split_off(at as usize), value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_model_code_is_picked_out_of_the_name_however_it_is_spelt() {
        let code = model_code;
        assert_eq!(code("Halot One CL-60").as_deref(), Some("CL-60"));
        assert_eq!(code("Halot Ray CL925").as_deref(), Some("CL-925"));
        assert_eq!(code("Halot Lite CL-89L").as_deref(), Some("CL-89L"));
        assert_eq!(code("CT-005 Pro").as_deref(), Some("CT-005"));
        assert_eq!(code("Mars 4 Ultra"), None, "another vendor's machine");
    }

    #[test]
    fn a_block_built_at_an_offset_holds_only_its_own_bytes() {
        let (bytes, value) = block_at(16, |fields| {
            let at = fields.position()?;
            fields.u32_le(42)?;
            Ok(at)
        })
        .expect("in memory");
        assert_eq!(value, 16, "a record addresses itself where it will land");
        assert_eq!(bytes, 42u32.to_le_bytes());
    }
}
