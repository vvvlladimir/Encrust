use std::fs::File;
use std::io::{BufReader, SeekFrom};
use std::path::Path;

use core_format::{FormatError, OpenFile, ReadSeek, SlicedFile, SlicedFileReader};
use core_raster::Run;
use format_anycubic::AnycubicReader;
use format_chitu::ChituReader;
use format_creality::{CxdlpReader, CxdlpV4Reader};
use format_cws::CwsReader;
use format_gcode_zip::GcodeZipReader;
use format_goo::GooReader;
use format_sl1::Sl1Reader;
use format_svgx::SvgxReader;

use crate::error::PipelineError;

/// A sliced file opened for inspection, whichever container it turned out to be.
///
/// One enum rather than a boxed trait object: every reader is in this workspace, and a
/// caller that wanted another would be adding a `format-*` crate beside them anyway. See
/// `docs/decisions/0149`.
pub enum Opened<S> {
    Goo(format_goo::OpenGoo<S>),
    Chitu(format_chitu::OpenChitu<S>),
    Anycubic(format_anycubic::OpenAnycubic<S>),
    Sl1(format_sl1::OpenSl1<S>),
    GcodeZip(format_gcode_zip::OpenGcodeZip<S>),
    Cxdlp(format_creality::OpenCxdlp<S>),
    CxdlpV4(format_creality::OpenCxdlpV4<S>),
    Svgx(format_svgx::OpenSvgx<S>),
    Cws(format_cws::OpenCws<S>),
}

impl<S: ReadSeek> OpenFile for Opened<S> {
    fn facts(&self) -> &SlicedFile {
        match self {
            Self::Goo(open) => open.facts(),
            Self::Chitu(open) => open.facts(),
            Self::Anycubic(open) => open.facts(),
            Self::Sl1(open) => open.facts(),
            Self::GcodeZip(open) => open.facts(),
            Self::Cxdlp(open) => open.facts(),
            Self::CxdlpV4(open) => open.facts(),
            Self::Svgx(open) => open.facts(),
            Self::Cws(open) => open.facts(),
        }
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        match self {
            Self::Goo(open) => open.layer(index),
            Self::Chitu(open) => open.layer(index),
            Self::Anycubic(open) => open.layer(index),
            Self::Sl1(open) => open.layer(index),
            Self::GcodeZip(open) => open.layer(index),
            Self::Cxdlp(open) => open.layer(index),
            Self::CxdlpV4(open) => open.layer(index),
            Self::Svgx(open) => open.layer(index),
            Self::Cws(open) => open.layer(index),
        }
    }
}

/// Opens a sliced file for inspection, reading its tables and no mask.
///
/// The extension chooses the reader, as it chooses the writer (ADR 0047). A name no reader
/// claims is decided by the first bytes instead, because a file renamed on the way out of
/// another slicer is still the container it was.
pub fn open<S: ReadSeek>(path: &Path, mut source: S) -> Result<Opened<S>, FormatError> {
    let family = match claimed_by(path) {
        Some(family) => family,
        None => sniff(&mut source)?,
    };
    family.open_from(source)
}

/// Opens the file at `path` and keeps it open, so its layers can be read for as long as the
/// returned value lives. This is what a window holds; see `docs/decisions/0150`.
pub fn open_file(path: &Path) -> Result<Opened<Box<dyn ReadSeek>>, PipelineError> {
    let file = File::open(path).map_err(|source| PipelineError::Read {
        path: path.to_owned(),
        source,
    })?;
    let source: Box<dyn ReadSeek> = Box::new(BufReader::new(file));
    open(path, source).map_err(|source| PipelineError::Open {
        path: path.to_owned(),
        source,
    })
}

/// The family the first bytes name.
///
/// Every container announces itself in its first sixteen bytes, so one read settles it and
/// no reader is run speculatively.
fn sniff<S: ReadSeek>(source: &mut S) -> Result<Family, FormatError> {
    let mut head = [0; 16];
    source.seek(SeekFrom::Start(0))?;
    let read = fill(source, &mut head)?;
    source.seek(SeekFrom::Start(0))?;
    let head = &head[..read];

    if head.starts_with(b"ANYCUBIC") {
        return Ok(Family::Anycubic);
    }
    if head.starts_with(b"PK") {
        // Three containers are zips, so the entries inside decide which this is.
        source.seek(SeekFrom::Start(0))?;
        let gcode_zip = format_gcode_zip::claims(source);
        source.seek(SeekFrom::Start(0))?;
        let cws = !gcode_zip && format_cws::claims(source);
        source.seek(SeekFrom::Start(0))?;
        return Ok(match (gcode_zip, cws) {
            (true, _) => Family::GcodeZip,
            (_, true) => Family::Cws,
            _ => Family::Sl1,
        });
    }
    if head.starts_with(b"DLP-II") {
        return Ok(Family::Svgx);
    }
    if head.len() >= 10 && &head[4..10] == b"CXSW3D" {
        return Ok(Family::Cxdlp);
    }
    if head.len() >= 12 && head[4..12] == GOO_MAGIC_TAG {
        return Ok(Family::Goo);
    }
    if head.len() >= 4 {
        let magic = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        if CHITU_MAGICS.contains(&magic) {
            return Ok(Family::Chitu);
        }
    }
    Err(FormatError::UnknownContainer)
}

/// The revision a `.cxdlp` states, which sits behind its magic.
fn cxdlp_version<S: ReadSeek>(source: &mut S) -> Result<u32, FormatError> {
    let mut field = [0; 2];
    source.seek(SeekFrom::Start(13))?;
    let read = fill(source, &mut field)?;
    source.seek(SeekFrom::Start(0))?;
    if read < field.len() {
        return Err(FormatError::UnknownContainer);
    }
    Ok(u32::from(u16::from_be_bytes(field)))
}

/// Reads as much of `buffer` as the file holds, which for a short file is not all of it.
fn fill<S: ReadSeek>(source: &mut S, buffer: &mut [u8]) -> Result<usize, FormatError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match source.read(&mut buffer[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}

/// The tag behind a `.goo`'s version string, and the magics of the Chitu family.
const GOO_MAGIC_TAG: [u8; 8] = [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00];
const CHITU_MAGICS: [u32; 3] = [0x12FD_0019, 0x12FD_0086, 0x12FD_0106];

/// Which container a reader handles, so one can be picked by name or by content.
#[derive(Debug, Clone, Copy)]
enum Family {
    Goo,
    Chitu,
    Anycubic,
    Sl1,
    GcodeZip,
    Cxdlp,
    Svgx,
    Cws,
}

impl Family {
    fn open_from<S: ReadSeek>(self, mut source: S) -> Result<Opened<S>, FormatError> {
        match self {
            Self::Goo => GooReader.open(source).map(Opened::Goo),
            Self::Chitu => ChituReader.open(source).map(Opened::Chitu),
            Self::Anycubic => AnycubicReader.open(source).map(Opened::Anycubic),
            Self::Sl1 => Sl1Reader.open(source).map(Opened::Sl1),
            Self::GcodeZip => GcodeZipReader.open(source).map(Opened::GcodeZip),
            // One magic heads both revisions and nothing else tells them apart, so the
            // field behind it picks the reader.
            Self::Svgx => SvgxReader.open(source).map(Opened::Svgx),
            Self::Cws => CwsReader.open(source).map(Opened::Cws),
            Self::Cxdlp => match cxdlp_version(&mut source)? {
                4 => CxdlpV4Reader.open(source).map(Opened::CxdlpV4),
                _ => CxdlpReader.open(source).map(Opened::Cxdlp),
            },
        }
    }
}

/// Whether a name is one a reader claims, which is how a front end tells a sliced file from
/// a mesh without opening it.
pub fn reads_sliced_file(path: &Path) -> bool {
    claimed_by(path).is_some()
}

/// The reader an extension names, or `None` for a name none of them claims.
fn claimed_by(path: &Path) -> Option<Family> {
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    match extension.as_str() {
        "goo" => Some(Family::Goo),
        "ctb" | "cbddlp" | "photon" => Some(Family::Chitu),
        "sl1" | "sl1s" => Some(Family::Sl1),
        "zip" => Some(Family::GcodeZip),
        "cxdlp" => Some(Family::Cxdlp),
        "svgx" => Some(Family::Svgx),
        "cws" => Some(Family::Cws),
        other => format_anycubic::AnycubicFlavour::of_extension(other).map(|_| Family::Anycubic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_extension_we_write_is_an_extension_we_can_open() {
        for name in [
            "a.goo", "a.ctb", "a.cbddlp", "a.photon", "a.pwmx", "a.pwmo", "a.dlp", "a.sl1",
            "a.sl1s", "a.zip", "a.cxdlp", "a.svgx", "a.cws",
        ] {
            assert!(
                claimed_by(Path::new(name)).is_some(),
                "{name} is a file this project writes"
            );
        }
    }

    #[test]
    fn an_extension_is_taken_whatever_its_case() {
        assert!(matches!(
            claimed_by(Path::new("a.CBDDLP")),
            Some(Family::Chitu)
        ));
    }

    #[test]
    fn a_name_no_reader_claims_is_left_to_the_content() {
        assert!(claimed_by(Path::new("a.stl")).is_none());
        assert!(claimed_by(Path::new("a")).is_none());
    }

    #[test]
    fn something_that_is_no_container_at_all_is_named_as_such() {
        let mut source = std::io::Cursor::new(vec![0x42; 512]);
        let err = open(Path::new("mystery.bin"), &mut source)
            .err()
            .expect("nothing claims it");
        assert!(matches!(err, FormatError::UnknownContainer));
    }

    #[test]
    fn a_renamed_file_is_recognised_by_its_first_bytes() {
        let mut anycubic = std::io::Cursor::new(b"ANYCUBIC\0\0\0\0".to_vec());
        assert!(matches!(sniff(&mut anycubic), Ok(Family::Anycubic)));

        let mut chitu = std::io::Cursor::new(0x12FD_0106u32.to_le_bytes().to_vec());
        assert!(matches!(sniff(&mut chitu), Ok(Family::Chitu)));

        let mut goo = std::io::Cursor::new([b"V3.0".as_slice(), &GOO_MAGIC_TAG].concat());
        assert!(matches!(sniff(&mut goo), Ok(Family::Goo)));

        let mut archive = std::io::Cursor::new(b"PK\x03\x04".to_vec());
        assert!(matches!(sniff(&mut archive), Ok(Family::Sl1)));
    }

    #[test]
    fn a_file_too_short_to_name_itself_is_not_guessed_at() {
        let mut source = std::io::Cursor::new(vec![0x07]);
        assert!(matches!(
            sniff(&mut source),
            Err(FormatError::UnknownContainer)
        ));
    }
}
