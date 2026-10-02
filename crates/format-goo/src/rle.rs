use core_raster::{LayerRuns, Run};

use core_format::FormatError;

/// Largest run one chunk can carry: the length field is 28 bits wide.
const MAX_CHUNK_RUN: u32 = 0x0FFF_FFFF;

/// Largest run a difference chunk can carry: its length is a single byte.
const MAX_DIFF_RUN: u32 = 255;

/// One layer's pixels in the run-length form `.goo` stores, with its checksum.
///
/// The bit layout is in `docs/formats/goo.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedLayer {
    width: u32,
    height: u32,
    data: Vec<u8>,
    checksum: u8,
}

impl EncodedLayer {
    /// Compresses a layer. Runs are taken across the whole image, not per row, so one
    /// that crosses the end of a row stays a single chunk.
    pub fn encode(layer: &LayerRuns) -> Self {
        let mut encoder = Encoder::default();
        for run in layer.runs() {
            encoder.add_run(run.length, run.value);
        }

        Self {
            width: layer.width(),
            height: layer.height(),
            checksum: checksum(&encoder.data),
            data: encoder.data,
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

    pub fn checksum(&self) -> u8 {
        self.checksum
    }

    /// Value of the layer's `data size` field: the magic byte, the runs and the checksum.
    pub fn data_size(&self) -> u32 {
        self.data.len() as u32 + 2
    }
}

/// The checksum the printer verifies: the negated sum of the run bytes.
///
/// It covers neither the leading magic byte nor the checksum byte itself. The
/// specification does not mention it at all; see `docs/formats/goo.md`.
pub fn checksum(data: &[u8]) -> u8 {
    !data.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte))
}

#[derive(Default)]
struct Encoder {
    data: Vec<u8>,
    last_value: u8,
}

impl Encoder {
    fn add_run(&mut self, length: u32, value: u8) {
        let mut left = length;
        while left > 0 {
            left -= self.add_chunk(left, value);
        }
    }

    /// Emits one chunk and reports how many pixels of the run it covered.
    fn add_chunk(&mut self, length: u32, value: u8) -> u32 {
        let difference = i16::from(value) - i16::from(self.last_value);
        let encodable_difference = !self.data.is_empty()
            && value != 0x00
            && value != 0xFF
            && difference.abs() <= 0x0F
            && length <= MAX_DIFF_RUN;

        if encodable_difference {
            self.push_difference(length, value, difference);
            return length;
        }

        let chunk_type: u8 = match value {
            0x00 => 0b00,
            0xFF => 0b11,
            _ => 0b01,
        };
        let taken = length.min(MAX_CHUNK_RUN);
        let size = match taken {
            0..=0xF => 0u8,
            0x10..=0xFFF => 1,
            0x1000..=0xF_FFFF => 2,
            _ => 3,
        };

        self.data
            .push((chunk_type << 6) | (size << 4) | (taken as u8 & 0x0F));
        if chunk_type == 0b01 {
            self.data.push(value);
        }
        for &shift in &[20u32, 12, 4][3 - size as usize..] {
            self.data.push((taken >> shift) as u8);
        }

        self.last_value = value;
        taken
    }

    fn push_difference(&mut self, length: u32, value: u8, difference: i16) {
        let head = (0b10 << 6)
            | (u8::from(difference < 0) << 5)
            | (u8::from(length != 1) << 4)
            | difference.unsigned_abs() as u8;
        self.data.push(head);
        if length != 1 {
            self.data.push(length as u8);
        }
        self.last_value = value;
    }
}

/// Expands run-length data back into runs.
///
/// Used to verify what was written and, from step 6, to draw the layer preview.
pub fn decode(data: &[u8]) -> Result<Vec<Run>, FormatError> {
    let mut runs = Vec::new();
    let mut value: u8 = 0;
    let mut cursor = 0;

    while cursor < data.len() {
        let offset = cursor;
        let head = data[cursor];
        cursor += 1;

        let length = if head >> 6 == 0b10 {
            value = apply_difference(value, head, offset)?;
            if head & 0b0001_0000 == 0 {
                1
            } else {
                u32::from(next(data, &mut cursor, offset)?)
            }
        } else {
            match head >> 6 {
                0b00 => value = 0x00,
                0b11 => value = 0xFF,
                _ => value = next(data, &mut cursor, offset)?,
            }
            read_length(data, &mut cursor, head, offset)?
        };

        // A zero-length run is what a truncated or corrupt chunk decodes to, and the
        // printer rejects the file over it.
        if length == 0 {
            return Err(FormatError::MalformedRun { offset });
        }
        runs.push(Run { length, value });
    }
    Ok(runs)
}

fn read_length(
    data: &[u8],
    cursor: &mut usize,
    head: u8,
    offset: usize,
) -> Result<u32, FormatError> {
    let size = ((head >> 4) & 0x03) as usize;
    let mut length = u32::from(head & 0x0F);
    for &shift in &[20u32, 12, 4][3 - size..] {
        length |= u32::from(next(data, cursor, offset)?) << shift;
    }
    Ok(length)
}

fn apply_difference(value: u8, head: u8, offset: usize) -> Result<u8, FormatError> {
    let magnitude = head & 0x0F;
    let shifted = if head & 0b0010_0000 == 0 {
        value.checked_add(magnitude)
    } else {
        value.checked_sub(magnitude)
    };
    shifted.ok_or(FormatError::MalformedRun { offset })
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
    use super::*;

    fn layer_from(width: u32, height: u32, pixels: &[u8]) -> LayerRuns {
        let mut mask = core_raster::LayerMask::new(width, height);
        mask.pixels_mut().copy_from_slice(pixels);
        LayerRuns::from_mask(&mask)
    }

    fn blank(width: u32, height: u32) -> LayerRuns {
        LayerRuns::builder(width, height).finish()
    }

    fn expanded(encoded: &EncodedLayer) -> Vec<u8> {
        decode(encoded.data())
            .expect("our own encoder produces decodable runs")
            .iter()
            .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
            .collect()
    }

    #[test]
    fn a_blank_layer_is_one_black_chunk() {
        let encoded = EncodedLayer::encode(&blank(4, 2));
        // Eight pixels fit the 4-bit length, so the whole layer is a single head byte.
        assert_eq!(encoded.data(), [0b0000_1000]);
        assert_eq!(encoded.data_size(), 3);
    }

    #[test]
    fn a_fully_lit_layer_is_one_white_chunk() {
        let layer = layer_from(4, 2, &[0xFF; 8]);
        assert_eq!(EncodedLayer::encode(&layer).data(), [0b1100_1000]);
    }

    #[test]
    fn a_grey_run_carries_its_value_after_the_head() {
        let layer = layer_from(4, 1, &[0x80; 4]);
        assert_eq!(EncodedLayer::encode(&layer).data(), [0b0100_0100, 0x80]);
    }

    #[test]
    fn a_small_step_in_grey_becomes_a_difference_chunk() {
        let layer = layer_from(4, 1, &[0x80, 0x84, 0x84, 0x84]);
        let encoded = EncodedLayer::encode(&layer);
        assert_eq!(
            encoded.data(),
            // 0x80 as a grey run of one, then +4 over three pixels.
            [0b0100_0001, 0x80, 0b1001_0100, 0x03]
        );
        assert_eq!(expanded(&encoded), layer.to_mask().pixels());
    }

    #[test]
    fn a_step_downwards_sets_the_sign_bit() {
        let layer = layer_from(2, 1, &[0x80, 0x7F]);
        let encoded = EncodedLayer::encode(&layer);
        assert_eq!(encoded.data(), [0b0100_0001, 0x80, 0b1010_0001]);
        assert_eq!(expanded(&encoded), layer.to_mask().pixels());
    }

    #[test]
    fn a_run_wider_than_four_bits_spills_into_extra_bytes() {
        let layer = layer_from(300, 1, &[0xFF; 300]);
        let encoded = EncodedLayer::encode(&layer);
        // 300 = 0x12C: low nibble 0xC in the head, 0x12 in the byte after it.
        assert_eq!(encoded.data(), [0b1101_1100, 0x12]);
        assert_eq!(expanded(&encoded), layer.to_mask().pixels());
    }

    #[test]
    fn the_checksum_is_the_negated_byte_sum() {
        assert_eq!(checksum(&[0x01, 0x02, 0x03]), !0x06u8);
        assert_eq!(checksum(&[]), 0xFF);
    }

    #[test]
    fn every_pixel_of_the_mask_survives_a_round_trip() {
        let pattern: Vec<u8> = (0..1024u32).map(|i| (i * 7 % 256) as u8).collect();
        let layer = layer_from(32, 32, &pattern);
        let encoded = EncodedLayer::encode(&layer);

        assert_eq!(expanded(&encoded), pattern);
        assert_eq!(encoded.width(), 32);
        assert_eq!(encoded.height(), 32);
    }

    #[test]
    fn a_grey_run_longer_than_a_difference_chunk_takes_the_literal_form() {
        let mut pixels = vec![0x40];
        pixels.extend(std::iter::repeat_n(0x44, 400));
        let layer = layer_from(401, 1, &pixels);
        let encoded = EncodedLayer::encode(&layer);

        // 400 pixels is past the 255 a difference chunk can hold, so the value is spelt out.
        assert_eq!(encoded.data()[2], 0b0101_0000);
        assert_eq!(expanded(&encoded), pixels);
    }

    #[test]
    fn truncated_data_is_reported_rather_than_guessed() {
        // A 12-bit length chunk whose second byte is missing.
        let err = decode(&[0b0001_0100]).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }

    #[test]
    fn a_zero_length_chunk_is_rejected() {
        let err = decode(&[0b0000_0000]).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }

    #[test]
    fn a_difference_running_past_black_is_rejected() {
        // First chunk leaves the colour at 0x01, the second subtracts 15 from it.
        let err = decode(&[0b0100_0001, 0x01, 0b1010_1111]).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 2 }));
    }
}
