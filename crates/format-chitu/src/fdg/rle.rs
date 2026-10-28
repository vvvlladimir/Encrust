use core_format::FormatError;
use core_raster::{LayerRuns, Run};

/// Most pixels one count byte carries; a longer stretch is split into several.
const MAX_COUNT: u32 = 0x7D;

/// Set on a byte that names a colour, which is itself one pixel.
const COLOUR_FLAG: u8 = 0x80;

/// The grey a lit pixel is written at. The container is binary, so this is the only one.
const LIT_GREY: u8 = 0x7C;

/// One layer as an `.fdg` stores it: one bit a pixel, cut at 128.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdgLayer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl FdgLayer {
    /// Compresses a layer, see `docs/formats/fdg.md` for the byte stream.
    pub fn encode(layer: &LayerRuns) -> Self {
        let mut stream = Stream::new(layer.width());
        for run in layer.runs() {
            stream.run(run.length, run.value >= 0x80, run.value);
        }
        Self {
            width: layer.width(),
            height: layer.height(),
            data: stream.finish(),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// The encoder's state over the whole layer, which runs on across rows.
struct Stream {
    width: u32,
    data: Vec<u8>,
    /// Index of the next pixel.
    at: u32,
    /// Pixels since the last colour byte that no count byte covers yet.
    pending: u32,
    /// The colour of the previous pixel; before the second pixel, the first one's raw grey.
    previous: u8,
}

impl Stream {
    fn new(width: u32) -> Self {
        Self {
            width,
            data: Vec::new(),
            at: 0,
            pending: 0,
            previous: 0,
        }
    }

    fn run(&mut self, length: u32, lit: bool, grey: u8) {
        let colour = if lit { 0xFF } else { 0x00 };
        let end = self.at + length;
        if self.at == 0 && length > 0 {
            // The first pixel is the opening byte, whose flag is set only for an unlit one.
            self.data.push(!grey & COLOUR_FLAG);
            self.previous = grey;
            self.at = 1;
        }
        while self.at < end {
            if colour != self.previous || self.is_boundary(self.at) {
                self.flush_count();
                self.data.push(COLOUR_FLAG | if lit { LIT_GREY } else { 0 });
                self.previous = colour;
                self.at += 1;
                continue;
            }
            let stop = self.next_boundary(self.at).min(end);
            self.count(stop - self.at);
            self.at = stop;
        }
    }

    /// The start of a row and its middle each open with a colour byte.
    fn is_boundary(&self, index: u32) -> bool {
        let column = index % self.width;
        column == 0 || column == self.width / 2
    }

    fn next_boundary(&self, index: u32) -> u32 {
        let column = index % self.width;
        let row_start = index - column;
        let half = self.width / 2;
        if column < half {
            row_start + half
        } else {
            row_start + self.width
        }
    }

    fn count(&mut self, pixels: u32) {
        self.pending += pixels;
        while self.pending >= MAX_COUNT {
            self.data.push(MAX_COUNT as u8);
            self.pending -= MAX_COUNT;
        }
    }

    fn flush_count(&mut self) {
        if self.pending > 0 {
            self.data.push(self.pending as u8);
            self.pending = 0;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        self.flush_count();
        self.data
    }
}

/// Expands one layer's bytes back into runs of eight-bit grey.
pub fn decode(data: &[u8], pixel_count: u32) -> Result<Vec<Run>, FormatError> {
    let mut runs: Vec<Run> = Vec::new();
    let mut grey = 0u8;
    let mut covered = 0u32;
    for (offset, &byte) in data.iter().enumerate() {
        let length = if offset == 0 && byte == 0 {
            // The opening byte of a lit first pixel carries no flag.
            grey = 255;
            1
        } else if byte & COLOUR_FLAG != 0 {
            let level = u32::from((byte & !COLOUR_FLAG).min(LIT_GREY));
            grey = (level * 255 / u32::from(LIT_GREY)) as u8;
            1
        } else {
            u32::from(byte)
        };
        if length == 0 {
            continue;
        }
        covered = covered
            .checked_add(length)
            .filter(|&covered| covered <= pixel_count)
            .ok_or(FormatError::MalformedRun { offset })?;
        match runs.last_mut() {
            Some(last) if last.value == grey => last.length += length,
            _ => runs.push(Run {
                length,
                value: grey,
            }),
        }
    }
    if covered < pixel_count {
        runs.push(Run {
            length: pixel_count - covered,
            value: 0,
        });
    }
    Ok(runs)
}

/// Covers or uncovers a layer's bytes with the keyed exclusive or; a key of zero leaves them
/// in the clear.
pub fn crypt(key: u32, index: u32, data: &mut [u8]) {
    if key == 0 {
        return;
    }
    let step = key.wrapping_add(0xE234_893D) ^ 0x257E_2431;
    let word = step.wrapping_mul((index ^ 0x110B_DACD).wrapping_mul(0x8239_1EFD));
    core_format::chitu_keystream_xor(data, word, step);
}
