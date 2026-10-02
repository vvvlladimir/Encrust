//! The seven-bit greyscale run-length codec the `.ctb` family and `.cxdlp` version 4 both
//! carry their layers in. See `docs/formats/chitu.md`.

use core_raster::{LayerRuns, Run};

use crate::FormatError;

/// Longest run one chunk can carry: the four-byte form has 28 bits of length.
const MAX_RUN: u32 = 0x0FFF_FFFF;

/// Set on the colour byte when a length follows it.
const RUN_FLAG: u8 = 0x80;

/// One layer in the seven-bit greyscale run-length form these containers store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rle7Layer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Rle7Layer {
    /// Compresses a layer.
    ///
    /// The format carries seven bits of grey, so each run's value is halved on the way in.
    /// Two runs whose eight-bit values differ can land on the same seven-bit value, and
    /// they have to be merged rather than written as two chunks: the decoder cannot tell
    /// them apart and a reader that compares run counts would disagree with us.
    pub fn encode(layer: &LayerRuns) -> Self {
        let mut data = Vec::new();
        let mut open: Option<(u8, u32)> = None;

        for run in layer.runs() {
            let grey7 = run.value >> 1;
            match open {
                Some((colour, length)) if colour == grey7 => {
                    open = Some((colour, length + run.length));
                }
                Some((colour, length)) => {
                    push_run(&mut data, colour, length);
                    open = Some((grey7, run.length));
                }
                None => open = Some((grey7, run.length)),
            }
        }
        if let Some((colour, length)) = open {
            push_run(&mut data, colour, length);
        }

        Self {
            width: layer.width(),
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

fn push_run(data: &mut Vec<u8>, colour: u8, length: u32) {
    let mut left = length;
    while left > 0 {
        let take = left.min(MAX_RUN);
        push_chunk(data, colour, take);
        left -= take;
    }
}

fn push_chunk(data: &mut Vec<u8>, colour: u8, length: u32) {
    if length == 1 {
        data.push(colour);
        return;
    }
    data.push(colour | RUN_FLAG);

    // The length's own prefix says how many bytes carry it, and those bytes are
    // big-endian even though every other field in the container is little-endian.
    match length {
        0..=0x7F => data.push(length as u8),
        0x80..=0x3FFF => data.extend_from_slice(&[(length >> 8) as u8 | 0x80, length as u8]),
        0x4000..=0x001F_FFFF => data.extend_from_slice(&[
            (length >> 16) as u8 | 0xC0,
            (length >> 8) as u8,
            length as u8,
        ]),
        _ => data.extend_from_slice(&[
            (length >> 24) as u8 | 0xE0,
            (length >> 16) as u8,
            (length >> 8) as u8,
            length as u8,
        ]),
    }
}

/// Expands a layer's run-length data back into runs of eight-bit grey.
///
/// A non-zero value is scaled back with its lowest bit set, which is what the reference
/// reader does, so full exposure decodes as 255 rather than 254 and black stays black.
pub fn decode_rle7(data: &[u8]) -> Result<Vec<Run>, FormatError> {
    let mut runs = Vec::new();
    let mut cursor = 0;

    while cursor < data.len() {
        let offset = cursor;
        let head = data[cursor];
        cursor += 1;
        let colour = head & !RUN_FLAG;
        let length = if head & RUN_FLAG == 0 {
            1
        } else {
            read_length(data, &mut cursor, offset)?
        };
        runs.push(Run {
            value: if colour == 0 { 0 } else { (colour << 1) | 1 },
            length,
        });
    }
    Ok(runs)
}

fn read_length(data: &[u8], cursor: &mut usize, offset: usize) -> Result<u32, FormatError> {
    let first = next(data, cursor, offset)?;

    // The leading ones of the first byte count the bytes that follow it, and the bits
    // under them are the length's own top bits. All four ones set is not a length.
    let (extra, mask) = if first & 0x80 == 0 {
        (0, 0x7F)
    } else if first & 0x40 == 0 {
        (1, 0x3F)
    } else if first & 0x20 == 0 {
        (2, 0x1F)
    } else if first & 0x10 == 0 {
        (3, 0x0F)
    } else {
        return Err(FormatError::MalformedRun { offset });
    };

    let mut length = u32::from(first & mask);
    for _ in 0..extra {
        length = (length << 8) | u32::from(next(data, cursor, offset)?);
    }
    Ok(length)
}

fn next(data: &[u8], cursor: &mut usize, offset: usize) -> Result<u8, FormatError> {
    let byte = data
        .get(*cursor)
        .copied()
        .ok_or(FormatError::MalformedRun { offset })?;
    *cursor += 1;
    Ok(byte)
}

#[cfg(test)]
mod tests {
    use core_raster::LayerMask;

    use super::*;

    fn runs_of(values: &[(u8, u32)]) -> LayerRuns {
        let total: u32 = values.iter().map(|(_, length)| length).sum();
        let mut mask = LayerMask::new(total, 1);
        let mut at = 0;
        for &(value, length) in values {
            for pixel in &mut mask.pixels_mut()[at..at + length as usize] {
                *pixel = value;
            }
            at += length as usize;
        }
        LayerRuns::from_mask(&mask)
    }

    #[test]
    fn a_single_pixel_is_one_byte_without_the_run_flag() {
        let encoded = Rle7Layer::encode(&runs_of(&[(0, 1)]));
        assert_eq!(encoded.data(), &[0x00]);
    }

    #[test]
    fn a_short_run_is_the_colour_then_one_length_byte() {
        // 255 >> 1 is 127, and the run flag makes that 0xFF.
        let encoded = Rle7Layer::encode(&runs_of(&[(255, 42)]));
        assert_eq!(encoded.data(), &[0xFF, 42]);
    }

    #[test]
    fn a_long_run_carries_its_length_big_endian_behind_a_prefix() {
        let encoded = Rle7Layer::encode(&runs_of(&[(0, 0x3FFF)]));
        assert_eq!(encoded.data(), &[0x80, 0xBF, 0xFF]);
    }

    #[test]
    fn two_values_that_share_a_seven_bit_grey_become_one_run() {
        // 200 and 201 both halve to 100, so the file must carry a single run of eight.
        let encoded = Rle7Layer::encode(&runs_of(&[(200, 4), (201, 4)]));
        assert_eq!(encoded.data(), &[0x64 | 0x80, 8]);
    }

    #[test]
    fn a_decoded_layer_has_the_run_lengths_it_was_given() {
        let encoded = Rle7Layer::encode(&runs_of(&[(0, 500), (255, 3), (0, 20_000)]));
        let decoded = decode_rle7(encoded.data()).expect("our own encoder is well formed");
        assert_eq!(
            decoded,
            vec![
                Run {
                    value: 0,
                    length: 500
                },
                Run {
                    value: 255,
                    length: 3
                },
                Run {
                    value: 0,
                    length: 20_000
                },
            ]
        );
    }

    #[test]
    fn a_length_prefix_no_encoder_writes_is_refused() {
        let err = decode_rle7(&[0xFF, 0xF0, 0x01, 0x02, 0x03]).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }

    #[test]
    fn a_length_that_runs_off_the_end_is_reported_with_its_offset() {
        let err = decode_rle7(&[0xFF, 0xC0, 0x01]).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }
}
