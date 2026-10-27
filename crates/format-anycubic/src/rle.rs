use core_format::FormatError;
use core_raster::{LayerRuns, Run};

/// Greys the format carries: one nibble per pixel, so sixteen steps of seventeen.
pub const GREY_STEPS: u16 = 16;

/// Longest run the two-byte form can carry: twelve bits of length.
const MAX_LONG_RUN: u32 = 0x0FFF;

/// Longest run the one-byte form can carry: four bits of length.
const MAX_SHORT_RUN: u32 = 0x000F;

/// One layer in the four-bit run-length form the Anycubic containers store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedLayer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl EncodedLayer {
    /// Compresses a layer.
    ///
    /// The format carries four bits of grey, so each run's value loses its low nibble. Two
    /// runs whose eight-bit values differ can land on the same nibble and have to be
    /// merged rather than written as two chunks: the decoder cannot tell them apart and a
    /// reader that compares run counts would disagree with us.
    pub fn encode(layer: &LayerRuns) -> Self {
        let mut data = Vec::new();
        let mut open: Option<(u8, u32)> = None;

        for run in layer.runs() {
            let grey4 = run.value >> 4;
            match open {
                Some((colour, length)) if colour == grey4 => {
                    open = Some((colour, length + run.length));
                }
                Some((colour, length)) => {
                    push_run(&mut data, colour, length);
                    open = Some((grey4, run.length));
                }
                None => open = Some((grey4, run.length)),
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

    /// Pixels this layer lights, which is what the layer table records beside it.
    ///
    /// Counted after the grey is quantised, so it is what the file cures rather than what
    /// the mask asked for: a pixel under 16 falls to nibble zero and lights nothing.
    pub fn lit_pixels(&self) -> u32 {
        let mut lit = 0;
        let mut cursor = 0;
        while cursor < self.data.len() {
            let colour = self.data[cursor] >> 4;
            let (length, step) = chunk_at(&self.data, cursor);
            if colour != 0 {
                lit += length;
            }
            cursor += step;
        }
        lit
    }
}

/// Black and white take the two-byte form, every grey between them the one-byte form.
///
/// A nibble of 0 or 0xF in the high nibble of a one-byte chunk could not be told from the
/// first byte of a two-byte one, which is why the format spends a whole word on them and
/// gets runs of 4095 back for it.
fn push_run(data: &mut Vec<u8>, colour: u8, length: u32) {
    let long = colour == 0 || colour == 0x0F;
    let limit = if long { MAX_LONG_RUN } else { MAX_SHORT_RUN };

    let mut left = length;
    while left > 0 {
        let take = left.min(limit);
        if long {
            // The word is big-endian, unlike every other field of the container.
            let word = (u16::from(colour) << 12) | take as u16;
            data.extend_from_slice(&word.to_be_bytes());
        } else {
            data.push(colour << 4 | take as u8);
        }
        left -= take;
    }
}

/// The run length one chunk carries, and the bytes it took.
fn chunk_at(data: &[u8], cursor: usize) -> (u32, usize) {
    let head = data[cursor];
    let colour = head >> 4;
    if colour == 0 || colour == 0x0F {
        let length = u32::from(u16::from_be_bytes([head, data[cursor + 1]]) & MAX_LONG_RUN as u16);
        (length, 2)
    } else {
        (u32::from(head & MAX_SHORT_RUN as u8), 1)
    }
}

/// Expands a layer's run-length data back into runs of eight-bit grey.
///
/// A nibble is repeated into both halves of the byte, which is what the reference reader
/// does, so `0x0F` decodes to 255 and `0x00` stays black. A value therefore comes back
/// within one step of seventeen of where it went in, above it as often as below.
pub fn decode(data: &[u8]) -> Result<Vec<Run>, FormatError> {
    let mut runs = Vec::new();
    let mut cursor = 0;

    while cursor < data.len() {
        let colour = data[cursor] >> 4;
        if (colour == 0 || colour == 0x0F) && cursor + 1 >= data.len() {
            return Err(FormatError::MalformedRun { offset: cursor });
        }
        let (length, step) = chunk_at(data, cursor);
        runs.push(Run {
            value: colour << 4 | colour,
            length,
        });
        cursor += step;
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(values: &[(u8, u32)]) -> LayerRuns {
        let total: u32 = values.iter().map(|(_, length)| length).sum();
        let mut builder = LayerRuns::builder(total, 1);
        for &(value, length) in values {
            builder.push(length, value);
        }
        builder.finish()
    }

    fn round_trip(values: &[(u8, u32)]) -> Vec<Run> {
        let encoded = EncodedLayer::encode(&layer(values));
        decode(encoded.data()).expect("our own encoder is well formed")
    }

    #[test]
    fn the_sixteen_greys_are_seventeen_apart() {
        let steps: Vec<u8> = (0..GREY_STEPS as u8).map(|n| n << 4 | n).collect();
        assert_eq!(steps.first(), Some(&0));
        assert_eq!(
            steps.last(),
            Some(&255),
            "a full nibble decodes to full white"
        );
        assert!(
            steps.windows(2).all(|pair| pair[1] - pair[0] == 17),
            "repeating a nibble into both halves spaces the steps evenly"
        );
    }

    #[test]
    fn full_exposure_survives_the_round_trip() {
        assert_eq!(
            round_trip(&[(255, 10)]),
            vec![Run {
                value: 255,
                length: 10
            }]
        );
    }

    #[test]
    fn black_and_white_take_two_bytes_and_a_grey_takes_one() {
        let encoded = EncodedLayer::encode(&layer(&[(255, 20), (128, 3), (0, 20)]));
        assert_eq!(
            encoded.data(),
            &[0xF0, 0x14, 0x83, 0x00, 0x14],
            "white as a word, grey 8 as a byte, then black as a word"
        );
    }

    #[test]
    fn a_grey_comes_back_as_its_own_nibble_repeated() {
        // 100 is 0x64; the low nibble is dropped and the high one repeated, giving 0x66.
        assert_eq!(
            round_trip(&[(100, 4)]),
            vec![Run {
                value: 0x66,
                length: 4
            }]
        );
    }

    #[test]
    fn a_grey_under_one_step_goes_dark() {
        assert_eq!(
            round_trip(&[(15, 4)]),
            vec![Run {
                value: 0,
                length: 4
            }]
        );
    }

    #[test]
    fn two_values_that_share_a_nibble_become_one_run() {
        let encoded = EncodedLayer::encode(&layer(&[(200, 2), (207, 2)]));
        assert_eq!(
            decode(encoded.data()).expect("well formed"),
            vec![Run {
                value: 0xCC,
                length: 4
            }],
            "both are nibble 12, so a reader must see one run"
        );
    }

    #[test]
    fn a_run_past_each_forms_limit_is_split() {
        let white = EncodedLayer::encode(&layer(&[(255, 5000)]));
        assert_eq!(white.data().len(), 4, "4095 then 905, two bytes each");
        assert_eq!(round_trip(&[(255, 5000)]).len(), 2);

        let grey = EncodedLayer::encode(&layer(&[(128, 40)]));
        assert_eq!(grey.data().len(), 3, "15, 15 then 10, one byte each");
    }

    #[test]
    fn only_the_lit_pixels_are_counted() {
        let encoded = EncodedLayer::encode(&layer(&[(255, 7), (15, 5), (128, 3)]));
        assert_eq!(
            encoded.lit_pixels(),
            10,
            "the five pixels that fell to nibble zero light nothing"
        );
    }

    #[test]
    fn a_word_cut_short_is_an_error() {
        let err = decode(&[0xF0]).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }
}
