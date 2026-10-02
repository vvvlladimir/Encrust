use core_format::FormatError;
use core_raster::{LayerRuns, Run};

/// Bilevel passes one `.cbddlp` layer is written in, and so the greys it can carry.
///
/// The container has no grey of its own: a machine counts the passes that light a pixel
/// and reads the count as its value. See `docs/formats/chitu.md`.
pub const GREY_PASSES: u32 = 8;

/// Longest run one byte can carry. Seven bits hold 127, but the reference writer stops at
/// 125 and firmware is only known to be tested against what that writes.
const MAX_RUN: u32 = 0x7D;

/// Set on a byte whose run is lit.
const LIT_FLAG: u8 = 0x80;

/// One layer as the bilevel passes `.cbddlp` and `.photon` store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPasses {
    width: u32,
    height: u32,
    passes: Vec<Vec<u8>>,
}

impl EncodedPasses {
    /// Compresses a layer into one pass per grey step the format carries.
    pub fn encode(layer: &LayerRuns) -> Self {
        let passes = (0..GREY_PASSES)
            .map(|pass| encode_pass(layer, threshold(pass)))
            .collect();

        Self {
            width: layer.width(),
            height: layer.height(),
            passes,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The passes in the order a machine counts them.
    pub fn passes(&self) -> &[Vec<u8>] {
        &self.passes
    }
}

/// Grey a pass lights a pixel from.
///
/// Pass zero wraps to 255, so only a fully lit pixel reaches every pass, and the count a
/// machine sums back scales to 255 rather than 256.
fn threshold(pass: u32) -> u8 {
    (256 / GREY_PASSES * pass).wrapping_sub(1) as u8
}

fn encode_pass(layer: &LayerRuns, threshold: u8) -> Vec<u8> {
    let mut data = Vec::new();
    let mut open: Option<(bool, u32)> = None;

    for run in layer.runs() {
        let lit = run.value >= threshold;
        match open {
            Some((was, length)) if was == lit => open = Some((lit, length + run.length)),
            Some((was, length)) => {
                push_run(&mut data, was, length);
                open = Some((lit, run.length));
            }
            None => open = Some((lit, run.length)),
        }
    }
    if let Some((lit, length)) = open {
        push_run(&mut data, lit, length);
    }
    data
}

fn push_run(data: &mut Vec<u8>, lit: bool, length: u32) {
    let mut left = length;
    while left > 0 {
        let take = left.min(MAX_RUN);
        data.push(take as u8 | if lit { LIT_FLAG } else { 0 });
        left -= take;
    }
}

/// Expands the passes of one layer back into runs of eight-bit grey.
///
/// A pixel's value is how many passes lit it, scaled by the step between two passes and
/// dropped by one so that every pass lighting it decodes to 255.
pub fn decode_passes(passes: &[Vec<u8>], pixel_count: u32) -> Result<Vec<Run>, FormatError> {
    let mut counts = vec![0u8; pixel_count as usize];

    for pass in passes {
        let mut at = 0usize;
        for (offset, byte) in pass.iter().enumerate() {
            let length = usize::from(byte & !LIT_FLAG);
            if at + length > counts.len() {
                return Err(FormatError::MalformedRun { offset });
            }
            if byte & LIT_FLAG != 0 {
                for count in &mut counts[at..at + length] {
                    *count += 1;
                }
            }
            at += length;
        }
    }

    let step = 256 / GREY_PASSES as u16;
    Ok(coalesce(counts.into_iter().map(|count| {
        let value = u16::from(count) * step;
        value.saturating_sub(1) as u8
    })))
}

fn coalesce(values: impl Iterator<Item = u8>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for value in values {
        match runs.last_mut() {
            Some(run) if run.value == value => run.length += 1,
            _ => runs.push(Run { value, length: 1 }),
        }
    }
    runs
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

    #[test]
    fn the_passes_climb_from_the_dimmest_grey_to_the_brightest() {
        let thresholds: Vec<u8> = (0..GREY_PASSES).map(threshold).collect();
        assert_eq!(
            thresholds,
            [255, 31, 63, 95, 127, 159, 191, 223],
            "pass zero wraps, so only a fully lit pixel is in every pass"
        );
    }

    #[test]
    fn full_exposure_survives_the_round_trip() {
        let runs = layer(&[(255, 10)]);
        let encoded = EncodedPasses::encode(&runs);
        let decoded =
            decode_passes(encoded.passes(), runs.pixel_count()).expect("the passes are sound");
        assert_eq!(
            decoded,
            vec![Run {
                value: 255,
                length: 10
            }]
        );
    }

    #[test]
    fn black_stays_black() {
        let runs = layer(&[(0, 8)]);
        let encoded = EncodedPasses::encode(&runs);
        assert_eq!(
            encoded.passes().len(),
            GREY_PASSES as usize,
            "every pass is written even when nothing is lit"
        );
        let decoded =
            decode_passes(encoded.passes(), runs.pixel_count()).expect("the passes are sound");
        assert_eq!(
            decoded,
            vec![Run {
                value: 0,
                length: 8
            }]
        );
    }

    #[test]
    fn a_grey_lands_on_the_step_below_it() {
        // 100 reaches the passes at 31, 63 and 95, so three of eight steps light it.
        let runs = layer(&[(100, 4)]);
        let encoded = EncodedPasses::encode(&runs);
        let decoded =
            decode_passes(encoded.passes(), runs.pixel_count()).expect("the passes are sound");
        assert_eq!(
            decoded,
            vec![Run {
                value: 95,
                length: 4
            }]
        );
    }

    #[test]
    fn a_run_longer_than_one_byte_is_split() {
        let runs = layer(&[(255, 300)]);
        let encoded = EncodedPasses::encode(&runs);
        let pass = &encoded.passes()[0];
        assert_eq!(pass.len(), 3, "300 pixels take three bytes at 125 each");
        assert_eq!(pass[0], 0x7D | LIT_FLAG);
        assert_eq!(pass[2], 0x32 | LIT_FLAG);
        let decoded =
            decode_passes(encoded.passes(), runs.pixel_count()).expect("the passes are sound");
        assert_eq!(
            decoded,
            vec![Run {
                value: 255,
                length: 300
            }]
        );
    }

    #[test]
    fn a_pass_that_runs_past_the_layer_is_an_error() {
        let err = decode_passes(&[vec![0x7D | LIT_FLAG]], 4).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }
}
