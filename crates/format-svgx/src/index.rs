//! One pass over the document that finds its settings and where each layer group stands.
//!
//! The document is the whole of a stack and is read as a stream: nothing of it is held but
//! the few hundred bytes of its settings and one offset per layer.

use core_format::{FormatError, ReadSeek, Reads};

/// Bytes read at a time while the document is walked.
const CHUNK_BYTES: usize = 64 * 1024;

/// How much of one tag is kept. A name and an attribute fit; the path data a group carries
/// does not need to.
const TAG_BYTES: usize = 1024;

/// Where one layer's group element stands in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Group {
    pub offset: u64,
    pub size: u32,
}

/// The settings elements as one slab of text, and the layer groups in document order.
pub(crate) fn scan<S: ReadSeek>(
    reads: &mut Reads<S>,
    document: u64,
    end: u64,
) -> Result<(String, Vec<Group>), FormatError> {
    reads.seek_to(document)?;
    let mut walk = Walk::default();
    let mut at = document;

    while at < end {
        let take = CHUNK_BYTES.min((end - at) as usize);
        let chunk = reads.bytes(take)?;
        for (index, &byte) in chunk.iter().enumerate() {
            walk.byte(byte, at + index as u64);
        }
        at += take as u64;
    }
    if walk.settings.is_empty() {
        return Err(FormatError::Missing {
            what: "the document's print parameters".to_owned(),
        });
    }
    Ok((walk.settings, walk.groups))
}

/// The state of the walk: which tag is being read, and which group is open.
#[derive(Default)]
struct Walk {
    settings: String,
    groups: Vec<Group>,
    /// The tag being read, its first byte's offset, and the quote it stands inside.
    tag: Option<(u64, Vec<u8>)>,
    quote: Option<u8>,
    /// Where the group now open began, and whether it is the background one.
    open: Option<(u64, bool)>,
}

impl Walk {
    fn byte(&mut self, byte: u8, at: u64) {
        let Some((start, tag)) = &mut self.tag else {
            if byte == b'<' {
                self.tag = Some((at, vec![b'<']));
            }
            return;
        };
        let keep = |tag: &mut Vec<u8>| {
            if tag.len() < TAG_BYTES {
                tag.push(byte);
            }
        };
        if let Some(quote) = self.quote {
            if byte == quote {
                self.quote = None;
            }
            keep(tag);
            return;
        }
        match byte {
            b'"' | b'\'' => {
                self.quote = Some(byte);
                keep(tag);
            }
            b'>' => {
                let (start, tag) = (*start, std::mem::take(tag));
                self.tag = None;
                self.end_tag(start, at, &String::from_utf8_lossy(&tag));
            }
            _ => keep(tag),
        }
    }

    /// One whole tag, from the `<` at `start` to the `>` at `at`. The sign itself is not
    /// in `tag`, so a closing tag reads `</g`.
    fn end_tag(&mut self, start: u64, at: u64, tag: &str) {
        if let Some(closing) = tag.strip_prefix("</") {
            if closing.trim() == "g"
                && let Some((from, background)) = self.open.take()
                && !background
            {
                self.groups.push(Group {
                    offset: from,
                    size: (at + 1 - from) as u32,
                });
            }
            return;
        }
        let name = tag
            .trim_start_matches('<')
            .split([' ', '\t', '\n', '/'])
            .next()
            .unwrap_or_default();
        match name {
            "printparams" | "projectiontime" | "projectionadjust" | "printrange" => {
                self.settings.push_str(tag);
                self.settings.push('\n');
            }
            "g" => self.open = Some((start, tag.contains("\"background\""))),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn scanned(document: &str) -> (String, Vec<Group>) {
        let mut source = Cursor::new(document.as_bytes().to_vec());
        let end = document.len() as u64;
        scan(&mut Reads::new(&mut source), 0, end).expect("the document holds its settings")
    }

    #[test]
    fn the_settings_of_every_element_land_in_one_slab() {
        let (settings, groups) = scanned(
            "<svg>\n<printparams layercount=\"2\">\n<projectiontime basetime=\"2.5\" />\n\
             </printparams>\n<g id=\"background\">\n</g>\n\
             <g id=\"layer-0\" area=\"1\">\n<path d=\"M 0 0 Z\" />\n</g>\n</svg>\n",
        );
        assert!(settings.contains("layercount=\"2\""));
        assert!(settings.contains("basetime=\"2.5\""));
        assert_eq!(groups.len(), 1, "the background group is not a layer");
    }

    #[test]
    fn a_group_runs_from_its_tag_to_its_closing_tag() {
        let document = "<printparams x=\"1\">\n</printparams>\n<g id=\"layer-0\">\n</g>\n";
        let (_, groups) = scanned(document);
        let group = groups[0];
        let text =
            &document[group.offset as usize..(group.offset + u64::from(group.size)) as usize];
        assert!(text.starts_with("<g id=\"layer-0\""));
        assert!(text.ends_with("</g>"));
    }

    #[test]
    fn a_tag_sign_inside_a_quoted_value_does_not_end_it() {
        let (settings, _) =
            scanned("<printparams machinename=\"a &gt; b\" x=\"2\">\n</printparams>");
        assert!(settings.contains("x=\"2\""), "{settings}");
    }

    #[test]
    fn a_document_without_settings_is_refused() {
        let mut source = Cursor::new(b"<svg>\n<g id=\"layer-0\">\n</g>\n</svg>".to_vec());
        let end = source.get_ref().len() as u64;
        let err = scan(&mut Reads::new(&mut source), 0, end)
            .expect_err("a document with no print parameters is not readable");
        assert!(matches!(err, FormatError::Missing { .. }));
    }
}
