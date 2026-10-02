use std::io::{self, Seek, SeekFrom, Write};

/// A sink a sliced file can be written to.
///
/// `Seek` is required of every format, not only the ones that need it: a container whose
/// header points at a table it precedes can only be written by going back over it. See
/// `docs/decisions/0045-sliced-files-are-written-to-a-seekable-sink.md`.
pub trait WriteSeek: Write + Seek {}

impl<T: Write + Seek + ?Sized> WriteSeek for T {}

/// Field writer for a sliced-file container.
///
/// Both endiannesses are spelt out at the call site because the two formats disagree:
/// `.goo` is big-endian throughout and `.ctb` is little-endian.
pub struct Fields<'w> {
    sink: &'w mut dyn WriteSeek,
    written: usize,
}

impl<'w> Fields<'w> {
    pub fn new(sink: &'w mut dyn WriteSeek) -> Self {
        Self { sink, written: 0 }
    }

    /// Bytes handed to the sink so far. A format with a fixed-size header depends on it.
    ///
    /// This counts writes, not file length: bytes that overwrote earlier ones after a
    /// `seek_to` are counted again.
    pub fn written(&self) -> usize {
        self.written
    }

    /// Offset the next byte will land at.
    pub fn position(&mut self) -> io::Result<u64> {
        self.sink.stream_position()
    }

    /// Moves to an absolute offset, to fill in a field whose value was not known yet.
    pub fn seek_to(&mut self, offset: u64) -> io::Result<()> {
        self.sink.seek(SeekFrom::Start(offset))?;
        Ok(())
    }

    pub fn bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.sink.write_all(bytes)?;
        self.written += bytes.len();
        Ok(())
    }

    pub fn u8(&mut self, value: u8) -> io::Result<()> {
        self.bytes(&[value])
    }

    pub fn u16_be(&mut self, value: u16) -> io::Result<()> {
        self.bytes(&value.to_be_bytes())
    }

    pub fn u32_be(&mut self, value: u32) -> io::Result<()> {
        self.bytes(&value.to_be_bytes())
    }

    pub fn f32_be(&mut self, value: f32) -> io::Result<()> {
        self.bytes(&value.to_be_bytes())
    }

    pub fn u16_le(&mut self, value: u16) -> io::Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    pub fn u32_le(&mut self, value: u32) -> io::Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    pub fn f32_le(&mut self, value: f32) -> io::Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    pub fn bool(&mut self, value: bool) -> io::Result<()> {
        self.u8(u8::from(value))
    }

    pub fn zeros(&mut self, count: usize) -> io::Result<()> {
        const CHUNK: [u8; 256] = [0; 256];
        let mut left = count;
        while left > 0 {
            let take = left.min(CHUNK.len());
            self.bytes(&CHUNK[..take])?;
            left -= take;
        }
        Ok(())
    }

    /// A fixed-width field holding text, padded with nul bytes and truncated to fit.
    pub fn text(&mut self, value: &str, width: usize) -> io::Result<()> {
        let mut end = value.len().min(width);
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        self.bytes(&value.as_bytes()[..end])?;
        self.zeros(width - end)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.sink.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn buffer() -> Cursor<Vec<u8>> {
        Cursor::new(Vec::new())
    }

    #[test]
    fn big_endian_integers_are_written_most_significant_byte_first() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        fields.u16_be(0x0102).expect("in-memory write");
        fields.u32_be(0x0304_0506).expect("in-memory write");
        assert_eq!(sink.into_inner(), [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }

    #[test]
    fn little_endian_integers_are_written_least_significant_byte_first() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        fields.u16_le(0x0102).expect("in-memory write");
        fields.u32_le(0x0304_0506).expect("in-memory write");
        assert_eq!(sink.into_inner(), [0x02, 0x01, 0x06, 0x05, 0x04, 0x03]);
    }

    #[test]
    fn text_is_padded_out_to_the_field_width() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        fields.text("ab", 5).expect("in-memory write");
        assert_eq!(fields.written(), 5);
        assert_eq!(sink.into_inner(), [b'a', b'b', 0, 0, 0]);
    }

    #[test]
    fn text_longer_than_the_field_is_cut_at_a_character_boundary() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        // "ю" is two bytes, so a four-byte field can only hold the first three characters.
        fields.text("abcю", 4).expect("in-memory write");
        assert_eq!(sink.into_inner(), [b'a', b'b', b'c', 0]);
    }

    #[test]
    fn a_long_run_of_zeros_is_written_in_full() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        fields.zeros(600).expect("in-memory write");
        let written = sink.into_inner();
        assert_eq!(written.len(), 600);
        assert!(written.iter().all(|&b| b == 0));
    }

    #[test]
    fn seeking_back_overwrites_a_field_left_for_later() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        let placeholder = fields.position().expect("in-memory seek");
        fields.u32_le(0).expect("in-memory write");
        fields.bytes(&[0xAA, 0xBB]).expect("in-memory write");
        let end = fields.position().expect("in-memory seek");

        fields.seek_to(placeholder).expect("in-memory seek");
        fields.u32_le(end as u32).expect("in-memory write");

        assert_eq!(sink.into_inner(), [0x06, 0x00, 0x00, 0x00, 0xAA, 0xBB]);
    }
}
