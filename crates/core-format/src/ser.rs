use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};

/// A sink a sliced file can be written to. `Seek` lets a header point at a table it precedes
/// (ADR 0045); `Read` lets a trailer hash what was written before it (ADR 0375).
pub trait ReadWriteSeek: Read + Write + Seek {}

impl<T: Read + Write + Seek + ?Sized> ReadWriteSeek for T {}

/// A file written through a buffer that can still be read back: what `BufWriter` cannot.
pub struct BufferedSink<F: Read + Write + Seek> {
    buffer: BufWriter<F>,
}

impl<F: Read + Write + Seek> BufferedSink<F> {
    /// `file` behind a buffer of `capacity` bytes.
    pub fn with_capacity(capacity: usize, file: F) -> Self {
        Self {
            buffer: BufWriter::with_capacity(capacity, file),
        }
    }

    /// The file, once what the buffer holds has reached it.
    pub fn into_inner(self) -> Result<F, io::IntoInnerError<BufWriter<F>>> {
        self.buffer.into_inner()
    }
}

impl<F: Read + Write + Seek> Read for BufferedSink<F> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.buffer.flush()?;
        self.buffer.get_mut().read(into)
    }
}

impl<F: Read + Write + Seek> Write for BufferedSink<F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }
}

impl<F: Read + Write + Seek> Seek for BufferedSink<F> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.buffer.seek(to)
    }
}

/// Field writer for a sliced-file container.
///
/// Both endiannesses are spelt out at the call site because the two formats disagree:
/// `.goo` is big-endian throughout and `.ctb` is little-endian.
pub struct Fields<'w> {
    sink: &'w mut dyn ReadWriteSeek,
    written: usize,
}

impl<'w> Fields<'w> {
    pub fn new(sink: &'w mut dyn ReadWriteSeek) -> Self {
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

    pub fn f64_be(&mut self, value: f64) -> io::Result<()> {
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

    /// Hands every byte from the start of the sink to its current end to `each`, a chunk at
    /// a time, and leaves the position at the end, where a trailer over them goes.
    pub fn read_back(&mut self, mut each: impl FnMut(&[u8])) -> io::Result<()> {
        let end = self.sink.seek(SeekFrom::End(0))?;
        self.sink.seek(SeekFrom::Start(0))?;
        let mut chunk = vec![0; READ_BACK_CHUNK_BYTES];
        let mut left = end;
        while left > 0 {
            let take = left.min(chunk.len() as u64) as usize;
            self.sink.read_exact(&mut chunk[..take])?;
            each(&chunk[..take]);
            left -= take as u64;
        }
        Ok(())
    }
}

/// How much of a sink `Fields::read_back` holds at once.
const READ_BACK_CHUNK_BYTES: usize = 1 << 20;

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
    fn reading_back_sees_every_byte_in_order_and_ends_at_the_end() {
        let mut sink = buffer();
        let mut fields = Fields::new(&mut sink);
        fields.bytes(&[1, 2, 3, 4]).expect("in-memory write");
        fields.seek_to(1).expect("in-memory seek");
        fields.u8(9).expect("in-memory write");

        let mut seen = Vec::new();
        fields
            .read_back(|chunk| seen.extend_from_slice(chunk))
            .expect("in-memory read");
        fields.u8(5).expect("in-memory write");

        assert_eq!(seen, [1, 9, 3, 4]);
        assert_eq!(sink.into_inner(), [1, 9, 3, 4, 5]);
    }

    #[test]
    fn a_buffered_sink_reads_back_what_it_still_holds() {
        let mut sink = BufferedSink::with_capacity(64, buffer());
        let mut fields = Fields::new(&mut sink);
        fields.bytes(&[7, 8]).expect("in-memory write");

        let mut seen = Vec::new();
        fields
            .read_back(|chunk| seen.extend_from_slice(chunk))
            .expect("in-memory read");
        assert_eq!(seen, [7, 8]);
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
