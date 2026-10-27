use std::io;

use core_format::{Fields, FormatError};
use core_raster::{LayerRuns, Run};

/// Grey from which a pixel is lit; the container holds one bit a pixel.
const THRESHOLD: u8 = 128;

/// Most pixels one byte carries.
const MAX_RUN: u64 = 128;

/// Bytes of the record in front of a layer's data.
pub(crate) const RECORD_BYTES: u64 = 0x1C;

/// The record's length word counts bits, and thirty-two more than the data holds.
const LENGTH_BIAS_BITS: u32 = 32;

/// One layer as a `.photons` stores it: one bit a pixel, cut at 128.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotonsLayer {
    width: u32,
    height: u32,
    /// Lit pixels the record states, which leaves out the layer's last stretch.
    lit_px: u32,
    data: Vec<u8>,
}

impl PhotonsLayer {
    /// Compresses a layer; the byte stream is in `docs/formats/photons.md`.
    pub fn encode(layer: &LayerRuns) -> Self {
        let mut data = Vec::new();
        let mut lit_px = 0u64;
        let mut open: Option<(bool, u64)> = None;
        for run in layer.runs().iter().filter(|run| run.length > 0) {
            let lit = run.value >= THRESHOLD;
            match open.as_mut() {
                Some((colour, length)) if *colour == lit => *length += u64::from(run.length),
                _ => {
                    // The vendor writer counts a stretch once the next one starts, so the
                    // last stretch of a layer is never counted; kept, being what it reads.
                    if let Some((colour, length)) = open.replace((lit, u64::from(run.length))) {
                        lit_px += if colour { length } else { 0 };
                        push_stretch(&mut data, colour, length);
                    }
                }
            }
        }
        if let Some((colour, length)) = open {
            push_stretch(&mut data, colour, length);
        }
        Self {
            width: layer.width(),
            height: layer.height(),
            lit_px: lit_px.min(u64::from(u32::MAX)) as u32,
            data,
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

    /// Writes the record and the data behind it.
    pub(crate) fn write(&self, fields: &mut Fields<'_>) -> io::Result<()> {
        fields.u32_be(self.lit_px)?;
        fields.zeros(8)?;
        fields.u32_be(self.width)?;
        fields.u32_be(self.height)?;
        fields.u32_be((self.data.len() as u32).wrapping_mul(8) + LENGTH_BIAS_BITS)?;
        fields.bytes(&scrambled(self.width as u16))?;
        fields.bytes(&scrambled(self.height as u16))?;
        fields.bytes(&self.data)
    }
}

/// Bytes of data behind a record whose length word is `bits`.
pub(crate) fn data_bytes(bits: u32) -> u32 {
    bits.saturating_sub(LENGTH_BIAS_BITS) / 8
}

/// A sixteen-bit side as the end of a record holds it: high byte first, each byte's bits
/// in reverse.
fn scrambled(value: u16) -> [u8; 2] {
    let [high, low] = value.to_be_bytes();
    [high.reverse_bits(), low.reverse_bits()]
}

/// Writes one stretch of a colour, `MAX_RUN` pixels a byte at most.
fn push_stretch(data: &mut Vec<u8>, lit: bool, mut length: u64) {
    while length > 0 {
        let pixels = length.min(MAX_RUN);
        data.push(((u8::from(lit) << 7) | (pixels - 1) as u8).reverse_bits());
        length -= pixels;
    }
}

/// Expands one layer's bytes back into runs of 0 and 255.
pub fn decode(data: &[u8], pixel_count: u32) -> Result<Vec<Run>, FormatError> {
    let mut runs: Vec<Run> = Vec::new();
    let mut covered = 0u32;
    for (offset, &byte) in data.iter().enumerate() {
        let byte = byte.reverse_bits();
        let value = if byte & 0x80 != 0 { u8::MAX } else { 0 };
        let length = u32::from(byte & 0x7F) + 1;
        covered = covered
            .checked_add(length)
            .filter(|&covered| covered <= pixel_count)
            .ok_or(FormatError::MalformedRun { offset })?;
        match runs.last_mut() {
            Some(last) if last.value == value => last.length += length,
            _ => runs.push(Run { length, value }),
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
