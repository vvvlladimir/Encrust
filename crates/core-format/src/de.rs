use std::io::{self, Read, Seek, SeekFrom};

use crate::FormatError;

/// A source a sliced file can be read from.
///
/// `Seek` is required of every format, as it is of a sink: a container whose header points
/// at a table behind it can only be read by jumping to it. A zip is read the same way.
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek + ?Sized> ReadSeek for T {}

/// Most bytes one entry of an archive container may expand to.
///
/// A zip states an entry's uncompressed size in its own directory; this is what a reader
/// of the archive containers believes instead. One greyscale mask of the largest panel
/// `MAX_PANEL_PX` allows is the largest thing an entry legitimately holds, and the image
/// an entry actually carries is well under that.
pub const MAX_ENTRY_BYTES: u64 = crate::MAX_PANEL_PX as u64;

/// Reads one entry of an archive, growing rather than trusting the size it states.
///
/// An entry past `MAX_ENTRY_BYTES` is refused instead of decompressed, so a small archive
/// cannot ask for the memory of a large one; see `docs/design/hostile-files.md`.
pub fn read_entry(source: &mut impl Read, name: &str) -> Result<Vec<u8>, FormatError> {
    let mut bytes = Vec::new();
    source
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(FormatError::Io)?;
    if bytes.len() as u64 > MAX_ENTRY_BYTES {
        return Err(FormatError::Encoding {
            what: "an archive entry",
            reason: format!("{name} expands past {MAX_ENTRY_BYTES} bytes"),
        });
    }
    Ok(bytes)
}

/// Field reader for a sliced-file container, the dual of `Fields`.
///
/// It owns its source rather than borrowing one, so an open file can be held for as long as
/// a caller wants to read layers out of it; see `docs/decisions/0150`. A `&mut File` is
/// itself a source, so a caller that only wants the tables passes one.
///
/// Both endiannesses are spelt out at the call site because the formats disagree: `.goo` is
/// big-endian throughout and the rest are little-endian.
pub struct Reads<S> {
    source: S,
    /// The length, measured on the first question and kept: a file being read does not
    /// grow, and measuring it costs two seeks that empty a `BufReader`'s buffer.
    end: Option<u64>,
}

impl<S: ReadSeek> Reads<S> {
    pub fn new(source: S) -> Self {
        Self { source, end: None }
    }

    /// Offset the next byte will come from.
    pub fn position(&mut self) -> io::Result<u64> {
        self.source.stream_position()
    }

    /// Bytes from the start of the source to its end.
    pub fn length(&mut self) -> io::Result<u64> {
        if let Some(end) = self.end {
            return Ok(end);
        }
        let at = self.source.stream_position()?;
        let end = self.source.seek(SeekFrom::End(0))?;
        self.source.seek(SeekFrom::Start(at))?;
        self.end = Some(end);
        Ok(end)
    }

    /// Moves to an absolute offset a field pointed at.
    ///
    /// Fails when the offset is past the end of the file, because a container whose header
    /// addresses a block that is not there is a truncated one, and reading on from a
    /// clamped position would report nonsense rather than a fault.
    pub fn seek_to(&mut self, offset: u64) -> Result<(), FormatError> {
        let end = self.length()?;
        if offset > end {
            return Err(FormatError::AddressPastEnd { offset, end });
        }
        self.source.seek(SeekFrom::Start(offset))?;
        Ok(())
    }

    pub fn skip(&mut self, count: u64) -> Result<(), FormatError> {
        let at = self.position()?;
        self.seek_to(at + count)
    }

    /// Bytes between the next one and the end of the source.
    pub fn remaining(&mut self) -> Result<u64, FormatError> {
        let at = self.position()?;
        Ok(self.length()?.saturating_sub(at))
    }

    /// Refuses a count the source is too small to hold, before a caller reserves for it.
    ///
    /// `each_bytes` is the least one of the claimed things costs in the file, so a header
    /// promising more rows than there are bytes behind them is refused rather than
    /// turned into an allocation; see `docs/design/hostile-files.md`.
    pub fn claim(
        &mut self,
        what: &'static str,
        count: u64,
        each_bytes: u64,
    ) -> Result<usize, FormatError> {
        let available = self.length()?;
        if count
            .checked_mul(each_bytes)
            .is_none_or(|needed| needed > available)
        {
            return Err(FormatError::ImpossibleCount {
                what,
                count,
                available,
            });
        }
        Ok(count as usize)
    }

    pub fn bytes(&mut self, count: usize) -> Result<Vec<u8>, FormatError> {
        let available = self.remaining()?;
        if count as u64 > available {
            return Err(FormatError::ImpossibleCount {
                what: "bytes",
                count: count as u64,
                available,
            });
        }
        let mut buffer = vec![0; count];
        self.source.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], FormatError> {
        let mut buffer = [0; N];
        self.source.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    pub fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16_be(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub fn u32_be(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn f32_be(&mut self) -> Result<f32, FormatError> {
        Ok(f32::from_be_bytes(self.array()?))
    }

    pub fn u16_le(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32_le(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn f32_le(&mut self) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    /// A fixed-width field holding text, with its nul padding and any trailing blanks cut.
    ///
    /// Invalid bytes become the replacement character rather than an error: a name shown to
    /// a user is not worth refusing a whole file over.
    pub fn text(&mut self, width: usize) -> Result<String, FormatError> {
        let bytes = self.bytes(width)?;
        let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(width);
        Ok(String::from_utf8_lossy(&bytes[..end]).trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn reads(bytes: &mut Cursor<Vec<u8>>) -> Reads<&mut Cursor<Vec<u8>>> {
        Reads::new(bytes)
    }

    #[test]
    fn each_endianness_reads_the_bytes_the_writer_laid_down() {
        let mut source = Cursor::new(vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let mut reads = reads(&mut source);
        assert_eq!(reads.u16_be().expect("in memory"), 0x0102);
        assert_eq!(reads.u32_le().expect("in memory"), 0x0605_0403);
    }

    #[test]
    fn text_stops_at_the_nul_that_pads_it() {
        let mut source = Cursor::new(vec![b'a', b'b', 0, 0, 0]);
        assert_eq!(reads(&mut source).text(5).expect("in memory"), "ab");
    }

    #[test]
    fn text_with_no_padding_fills_the_field() {
        let mut source = Cursor::new(b"abcd".to_vec());
        assert_eq!(reads(&mut source).text(4).expect("in memory"), "abcd");
    }

    #[test]
    fn an_address_past_the_end_is_refused_rather_than_clamped() {
        let mut source = Cursor::new(vec![0; 8]);
        let err = reads(&mut source).seek_to(9).unwrap_err();
        assert!(matches!(
            err,
            FormatError::AddressPastEnd { offset: 9, end: 8 }
        ));
    }

    #[test]
    fn an_archive_entry_that_keeps_expanding_is_cut_off_rather_than_followed() {
        let mut endless = io::repeat(0);
        let err = read_entry(&mut endless, "bomb.png").unwrap_err();
        assert!(
            matches!(err, FormatError::Encoding { what, .. } if what == "an archive entry"),
            "{err}"
        );
    }

    #[test]
    fn a_length_longer_than_the_file_is_refused_before_it_is_allocated() {
        let mut source = Cursor::new(vec![0; 8]);
        let err = reads(&mut source).bytes(u32::MAX as usize).unwrap_err();
        assert!(matches!(
            err,
            FormatError::ImpossibleCount {
                available: 8,
                count: 4_294_967_295,
                ..
            }
        ));
    }

    #[test]
    fn a_row_count_the_file_cannot_hold_is_refused() {
        let mut source = Cursor::new(vec![0; 64]);
        let err = reads(&mut source).claim("layers", 10, 32).unwrap_err();
        assert!(matches!(
            err,
            FormatError::ImpossibleCount {
                what: "layers",
                count: 10,
                available: 64
            }
        ));
    }

    #[test]
    fn a_row_count_whose_product_overflows_is_refused_rather_than_wrapped() {
        let mut source = Cursor::new(vec![0; 64]);
        assert!(
            reads(&mut source)
                .claim("layers", u64::MAX, 32)
                .is_err_and(|err| matches!(err, FormatError::ImpossibleCount { .. }))
        );
    }

    #[test]
    fn a_count_the_file_does_hold_comes_back_as_a_capacity() {
        let mut source = Cursor::new(vec![0; 64]);
        let count = reads(&mut source)
            .claim("layers", 2, 32)
            .expect("two 32-byte rows fit a 64-byte file");
        assert_eq!(count, 2);
    }

    #[test]
    fn a_field_cut_short_by_the_end_of_the_file_is_an_error() {
        let mut source = Cursor::new(vec![0x01, 0x02]);
        assert!(matches!(
            reads(&mut source).u32_le().unwrap_err(),
            FormatError::Io(_)
        ));
    }
}
