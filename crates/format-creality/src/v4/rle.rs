//! The version 4 layer codec: runs of six-bit grey that restart on every row. Layout is in
//! `docs/formats/creality.md`.

use core_format::FormatError;
use core_raster::{LayerRuns, Run};

/// The flag on the byte that opens a run; its low bits are the grey.
const RUN: u8 = 0x80;
/// Bits of a length byte; the length is stored less one, most significant group first.
const GROUP_BITS: u32 = 7;
const GROUP_MASK: u8 = 0x7F;
/// The highest grey a run states: six bits, of which `0x3F` is fully lit.
const MAX_GREY: u8 = 0x3F;

/// A layer in the version 4 codec.
#[derive(Debug, Clone)]
pub struct RowRunsLayer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl RowRunsLayer {
    pub fn encode(layer: &LayerRuns) -> Self {
        let width = layer.width();
        let mut data = Vec::new();
        let mut pending: Option<(u8, u32)> = None;
        let mut column = 0;
        for run in layer.runs() {
            let grey = run.value >> 2;
            let mut left = run.length;
            while left > 0 {
                let take = left.min(width - column);
                pending = match pending {
                    Some((value, length)) if value == grey => Some((value, length + take)),
                    other => {
                        if let Some((value, length)) = other {
                            push_run(&mut data, value, length);
                        }
                        Some((grey, take))
                    }
                };
                column += take;
                left -= take;
                if column == width {
                    if let Some((value, length)) = pending.take() {
                        push_run(&mut data, value, length);
                    }
                    column = 0;
                }
            }
        }
        if let Some((value, length)) = pending {
            push_run(&mut data, value, length);
        }
        Self {
            width,
            height: layer.height(),
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
}

/// One run: its grey, then its length less one in seven-bit groups, none for a single pixel.
fn push_run(data: &mut Vec<u8>, grey: u8, length: u32) {
    data.push(RUN | grey.min(MAX_GREY));
    let stored = length - 1;
    if stored == 0 {
        return;
    }
    let groups = (u32::BITS - stored.leading_zeros()).div_ceil(GROUP_BITS);
    for group in (0..groups).rev() {
        data.push((stored >> (group * GROUP_BITS)) as u8 & GROUP_MASK);
    }
}

/// Expands a layer back into runs of eight-bit grey, a six-bit grey `g` reading as
/// `g << 2 | g >> 4`, so `0x3F` is 255.
pub fn decode(data: &[u8]) -> Result<Vec<Run>, FormatError> {
    let mut runs: Vec<Run> = Vec::new();
    let mut cursor = 0;
    while cursor < data.len() {
        let head = data[cursor];
        if head & RUN == 0 {
            return Err(FormatError::MalformedRun { offset: cursor });
        }
        cursor += 1;
        let mut stored: u32 = 0;
        while cursor < data.len() && data[cursor] & RUN == 0 {
            stored = stored
                .checked_mul(1 << GROUP_BITS)
                .ok_or(FormatError::MalformedRun { offset: cursor })?
                | u32::from(data[cursor]);
            cursor += 1;
        }
        let grey = head & MAX_GREY;
        let value = grey << 2 | grey >> 4;
        let length = stored + 1;
        match runs.last_mut() {
            Some(last) if last.value == value => last.length += length,
            _ => runs.push(Run { length, value }),
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(width: u32, height: u32, runs: &[(u32, u8)]) -> LayerRuns {
        let mut builder = LayerRuns::builder(width, height);
        for &(length, value) in runs {
            builder.push(length, value);
        }
        builder.finish()
    }

    #[test]
    fn a_dark_row_of_the_vendors_panel_is_its_three_bytes() {
        let encoded = RowRunsLayer::encode(&layer(15120, 1, &[(15120, 0)]));
        assert_eq!(encoded.data(), [0x80, 0x76, 0x0F]);
    }

    #[test]
    fn a_run_restarts_on_every_row() {
        let encoded = RowRunsLayer::encode(&layer(4, 2, &[(8, 255)]));
        assert_eq!(encoded.data(), [0xBF, 0x03, 0xBF, 0x03]);
    }

    #[test]
    fn a_single_pixel_takes_no_length() {
        let encoded = RowRunsLayer::encode(&layer(3, 1, &[(1, 0), (1, 255), (1, 0)]));
        assert_eq!(encoded.data(), [0x80, 0xBF, 0x80]);
    }

    #[test]
    fn a_layer_reads_back_in_six_bit_grey() {
        let source = layer(5, 2, &[(3, 0), (4, 255), (2, 128), (1, 0)]);
        let runs = decode(RowRunsLayer::encode(&source).data()).expect("well formed");
        let expected = [(3, 0), (4, 255), (2, 130), (1, 0)];
        let got: Vec<(u32, u8)> = runs.iter().map(|run| (run.length, run.value)).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn a_length_byte_with_no_run_before_it_is_refused() {
        assert!(matches!(
            decode(&[0x05]),
            Err(FormatError::MalformedRun { offset: 0 })
        ));
    }
}
