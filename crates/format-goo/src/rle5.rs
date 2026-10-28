use core_format::FormatError;
use core_raster::{LayerRuns, Run};

use crate::rle::checksum;

/// Longest run one length field carries: its value is stored in 28 bits.
const MAX_RUN: u32 = 0x0FFF_FFFF;

/// The short form's length field: 1 to 7 pixels in bits 3 to 5.
const SHORT_RUN_MAX: u32 = 7;

/// One layer of a version 5.1 file: the left and the right half of the panel, each in the
/// eight-level run-length form the revision stores. Bit layout in `docs/formats/goo.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedHalves {
    width: u32,
    height: u32,
    halves: [EncodedHalf; 2],
}

/// One half's runs and the checksum behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedHalf {
    pub(crate) data: Vec<u8>,
    pub(crate) checksum: u8,
}

impl EncodedHalves {
    /// Splits each row of `layer` at the middle and compresses both halves; the writer
    /// refuses a panel of odd width, which has no middle.
    pub fn encode(layer: &LayerRuns) -> Self {
        let (width, half) = (layer.width(), layer.width() / 2);
        let mut encoders = [Encoder::default(), Encoder::default()];
        let mut column = 0;
        for run in layer.runs() {
            let mut left = run.length;
            while left > 0 {
                let (side, edge) = if column < half { (0, half) } else { (1, width) };
                let taken = left.min(edge - column);
                encoders[side].push(run.value >> 5, taken);
                column = (column + taken) % width;
                left -= taken;
            }
        }
        Self {
            width,
            height: layer.height(),
            halves: encoders.map(Encoder::finish),
        }
    }

    /// Width of the whole panel, both halves, px.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Rows of the panel, px.
    pub fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn halves(&self) -> &[EncodedHalf; 2] {
        &self.halves
    }
}

#[derive(Default)]
struct Encoder {
    data: Vec<u8>,
    level: u8,
    pending: Option<(u8, u32)>,
}

impl Encoder {
    fn push(&mut self, level: u8, length: u32) {
        match &mut self.pending {
            Some((pending, total)) if *pending == level && *total <= MAX_RUN - length => {
                *total += length;
            }
            _ => {
                self.flush();
                self.pending = Some((level, length));
            }
        }
    }

    fn flush(&mut self) {
        let Some((level, length)) = self.pending.take() else {
            return;
        };
        if level != self.level {
            // A step of -7 to +7, offset by 0x20 under the high bit.
            let step = i16::from(level) - i16::from(self.level);
            self.data.push(0x80 | (step + 0x20) as u8);
            self.level = level;
            if length > 1 {
                self.push_length(length - 1);
            }
        } else if length <= SHORT_RUN_MAX {
            self.data.push(level | (length as u8) << 3);
        } else {
            self.data.push(level | 0x38);
            self.push_length(length - SHORT_RUN_MAX);
        }
    }

    /// The length field, one less than `count`: its low four bits in the head byte, the
    /// rest a byte each, least significant first.
    fn push_length(&mut self, count: u32) {
        let stored = count - 1;
        let low = (stored & 0x0F) as u8;
        let extra = match stored {
            0..=0xF => 0,
            0x10..=0xFFF => 1,
            0x1000..=0xF_FFFF => 2,
            _ => 3,
        };
        self.data.push(0x40 | extra << 4 | low);
        for byte in 0..extra {
            self.data.push((stored >> (4 + 8 * byte)) as u8);
        }
    }

    fn finish(mut self) -> EncodedHalf {
        self.flush();
        EncodedHalf {
            checksum: checksum(&self.data),
            data: self.data,
        }
    }
}

/// The grey a stored level stands for: 0 to 7 spread over the whole byte.
fn grey_of(level: u8) -> u8 {
    ((u16::from(level) * 255 + 3) / 7) as u8
}

/// Expands one half's runs, the checksum and magic already taken off.
pub fn decode_half(data: &[u8]) -> Result<Vec<Run>, FormatError> {
    let mut runs = Vec::new();
    let (mut level, mut cursor) = (0u8, 0);
    while cursor < data.len() {
        let offset = cursor;
        let head = data[cursor];
        cursor += 1;
        let length = match head {
            0x80..=0xFF => {
                let step = i16::from(head & 0x7F) - 0x20;
                level = u8::try_from(i16::from(level) + step)
                    .ok()
                    .filter(|&next| next <= 7)
                    .ok_or(FormatError::MalformedRun { offset })?;
                trailing_length(data, &mut cursor, offset)?.map_or(1, |stored| stored + 2)
            }
            0x00..=0x3F if head & 0x07 == level => match u32::from(head >> 3) {
                SHORT_RUN_MAX => trailing_length(data, &mut cursor, offset)?
                    .map_or(SHORT_RUN_MAX, |stored| stored + SHORT_RUN_MAX + 1),
                0 => return Err(FormatError::MalformedRun { offset }),
                short => short,
            },
            _ => return Err(FormatError::MalformedRun { offset }),
        };
        runs.push(Run {
            length,
            value: grey_of(level),
        });
    }
    Ok(runs)
}

/// The length field after a head byte, if one follows: only it starts with `01`.
fn trailing_length(
    data: &[u8],
    cursor: &mut usize,
    offset: usize,
) -> Result<Option<u32>, FormatError> {
    let Some(&head) = data.get(*cursor).filter(|&&byte| byte & 0xC0 == 0x40) else {
        return Ok(None);
    };
    *cursor += 1;
    let mut stored = u32::from(head & 0x0F);
    for byte in 0..(head >> 4 & 0x03) {
        let next = data
            .get(*cursor)
            .ok_or(FormatError::MalformedRun { offset })?;
        stored |= u32::from(*next) << (4 + 8 * u32::from(byte));
        *cursor += 1;
    }
    Ok(Some(stored))
}

/// The whole panel from its two halves, `half_px` wide and `height_px` tall, row by row:
/// the left half's row, then the right's.
pub fn join_halves(
    left: &[Run],
    right: &[Run],
    half_px: u32,
    height_px: u32,
) -> Result<Vec<Run>, FormatError> {
    let mut halves = [Taker::new(left), Taker::new(right)];
    let mut joined: Vec<Run> = Vec::with_capacity(left.len() + right.len());
    for _ in 0..height_px {
        for half in &mut halves {
            half.take(half_px, &mut joined)?;
        }
    }
    Ok(joined)
}

/// Hands out a run list a given number of pixels at a time.
struct Taker<'r> {
    runs: &'r [Run],
    index: usize,
    used: u32,
}

impl<'r> Taker<'r> {
    fn new(runs: &'r [Run]) -> Self {
        Self {
            runs,
            index: 0,
            used: 0,
        }
    }

    /// Appends the next `count` pixels to `into`, joining a run that carries on the last.
    fn take(&mut self, mut count: u32, into: &mut Vec<Run>) -> Result<(), FormatError> {
        while count > 0 {
            let run = self
                .runs
                .get(self.index)
                .ok_or(FormatError::MalformedRun { offset: self.index })?;
            let taken = count.min(run.length - self.used);
            match into.last_mut() {
                Some(last) if last.value == run.value => last.length += taken,
                _ => into.push(Run {
                    length: taken,
                    value: run.value,
                }),
            }
            self.used += taken;
            count -= taken;
            if self.used == run.length {
                (self.index, self.used) = (self.index + 1, 0);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
