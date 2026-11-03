//! The zip around a 3MF package's parts: a part by its name, and the root model part the
//! package's relationships name.

use std::io::{BufReader, Read};

use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::Event;
use zip::ZipArchive;

use super::model::Model;
use super::parse::parse;
use crate::ReadSeek;

/// The relationship type naming the root model part, the package's `StartPart` (core §2.1.5).
const START_PART: &[u8] = b"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel";

/// Where a package keeps its root model when its relationships do not say.
const USUAL_ROOT: &str = "3D/3dmodel.model";

/// Most bytes one part of the container may say it expands to. Every zip reader sizes a buffer from
/// the stated length; see `docs/design/hostile-files.md`.
const MAX_PART_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Most bytes deflate can expand one byte into; a 3MF package stores or deflates its parts.
const MAX_DEFLATE_RATIO: u64 = 1032;

pub(super) struct Package<'a> {
    archive: ZipArchive<BufReader<&'a mut dyn ReadSeek>>,
}

impl<'a> Package<'a> {
    /// Opens the archive, refusing one claiming a part larger than `MAX_PART_BYTES` or than
    /// `file_bytes` could inflate to, before any part is read.
    pub(super) fn open(source: &'a mut dyn ReadSeek, file_bytes: u64) -> Result<Self, String> {
        let most = MAX_PART_BYTES.min(file_bytes.saturating_mul(MAX_DEFLATE_RATIO));
        let mut archive =
            ZipArchive::new(BufReader::new(source)).map_err(|error| error.to_string())?;
        for index in 0..archive.len() {
            let entry = archive
                .by_index_raw(index)
                .map_err(|error| error.to_string())?;
            if entry.size() > most {
                return Err(format!(
                    "the part {} claims {} bytes",
                    String::from_utf8_lossy(entry.name_raw()),
                    entry.size()
                ));
            }
        }
        Ok(Self { archive })
    }

    /// The root model part's name, as `_rels/.rels` gives it.
    pub(super) fn root_name(&mut self) -> Result<String, String> {
        let Some(relationships) = self.bytes("_rels/.rels")? else {
            return Ok(USUAL_ROOT.to_owned());
        };
        Ok(start_part(&relationships)?.unwrap_or_else(|| USUAL_ROOT.to_owned()))
    }

    /// The model part `name` read whole; `root` as [`parse`] takes it.
    pub(super) fn model(&mut self, name: &str, root: bool) -> Result<Model, String> {
        let index = self
            .index(name)
            .ok_or_else(|| format!("no model part at {name}"))?;
        let part = self
            .archive
            .by_index(index)
            .map_err(|error| format!("{name}: {error}"))?;
        parse(BufReader::new(part), root).map_err(|reason| format!("{name}: {reason}"))
    }

    /// The bytes of the part `name`, or `None` when the package has no such part.
    pub(super) fn bytes(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(index) = self.index(name) else {
            return Ok(None);
        };
        let mut part = self
            .archive
            .by_index(index)
            .map_err(|error| format!("{name}: {error}"))?;
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes)
            .map_err(|error| format!("{name}: {error}"))?;
        Ok(Some(bytes))
    }

    /// The entry holding part `name`, as written or with its percent-escapes decoded; part names
    /// compare case-insensitively, zip entry names exactly.
    fn index(&self, name: &str) -> Option<usize> {
        let name = name.trim_start_matches('/');
        [Some(name.to_owned()), unescaped(name)]
            .into_iter()
            .flatten()
            .find_map(|name| {
                self.archive.index_for_name(&name).or_else(|| {
                    self.archive
                        .file_names_raw()
                        .position(|entry| entry.eq_ignore_ascii_case(name.as_bytes()))
                })
            })
    }
}

/// A part name the way the walk keys it: no leading slash, ASCII case folded.
pub(super) fn part_name(name: &str) -> String {
    name.trim_start_matches('/').to_ascii_lowercase()
}

/// `name` with its percent-escapes decoded, when it has any and they decode to UTF-8.
fn unescaped(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    if !bytes.contains(&b'%') {
        return None;
    }
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| bytes.get(at + 1..at + 3))
            .flatten()
            .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        if let Some(byte) = escaped {
            decoded.push(byte);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// The target of the `StartPart` relationship in a `.rels` part, if it names one.
fn start_part(relationships: &[u8]) -> Result<Option<String>, String> {
    let mut reader = Reader::from_reader(relationships);
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| format!("/_rels/.rels: {error}"))?
        {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == b"Relationship" =>
            {
                let kind = element.try_get_attribute("Type").ok().flatten();
                let target = element.try_get_attribute("Target").ok().flatten();
                if let (Some(kind), Some(target)) = (kind, target)
                    && kind.value.as_ref() == START_PART
                {
                    let target = target
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|error| format!("/_rels/.rels: {error}"))?;
                    return Ok(Some(target.into_owned()));
                }
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
        buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_name_is_unescaped_only_where_it_carries_escapes() {
        assert_eq!(
            unescaped("3D/My%20Model.model").as_deref(),
            Some("3D/My Model.model")
        );
        assert_eq!(unescaped("3D/3dmodel.model"), None);
        assert_eq!(
            unescaped("3D/100%.png").as_deref(),
            Some("3D/100%.png"),
            "a percent sign escaping nothing stays"
        );
    }
}
