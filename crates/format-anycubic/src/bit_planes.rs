//! The `.pws` layer codec: one one-bit pass of the panel per anti-alias level, summed into a
//! grey. See *The `.pws` codec* in `docs/formats/anycubic.md`.

use core_format::FormatError;
use core_raster::Run;

/// The most passes a file may ask for: the levels the format offers run 1, 2, 4, 8, 16.
pub(crate) const MAX_LEVELS: u32 = 16;

/// Sums `levels` passes over a panel of `panel_px` pixels into runs of eight-bit grey.
///
/// Each byte is a run: the high bit lights it, the low seven count 0 to 127 pixels. A pass
/// ends on the byte that fills the panel, and the next starts on the byte after it.
pub(crate) fn decode(data: &[u8], panel_px: u64, levels: u32) -> Result<Vec<Run>, FormatError> {
    let mut counts = vec![(panel_px, 0u8)];
    let mut cursor = 0;
    for _ in 0..levels {
        let (plane, read) = plane(&data[cursor.min(data.len())..], panel_px, cursor)?;
        counts = add(&counts, &plane);
        cursor += read;
    }
    Ok(counts
        .into_iter()
        .filter(|(length, _)| *length > 0)
        .map(|(length, count)| Run {
            length: u32::try_from(length).unwrap_or(u32::MAX),
            value: grey(count, levels),
        })
        .collect())
}

/// One pass as runs of lit or unlit, and the bytes it took. `start` is where it began in
/// the layer, for the error.
fn plane(
    data: &[u8],
    panel_px: u64,
    start: usize,
) -> Result<(Vec<(u64, bool)>, usize), FormatError> {
    let mut runs: Vec<(u64, bool)> = Vec::new();
    let mut filled = 0;
    for (index, byte) in data.iter().enumerate() {
        let (length, lit) = (u64::from(byte & 0x7F), byte & 0x80 != 0);
        if length > panel_px - filled {
            return Err(FormatError::MalformedRun {
                offset: start + index,
            });
        }
        match runs.last_mut() {
            Some((last, was_lit)) if *was_lit == lit => *last += length,
            _ => runs.push((length, lit)),
        }
        filled += length;
        if filled == panel_px {
            return Ok((runs, index + 1));
        }
    }
    Err(FormatError::MalformedRun {
        offset: start + data.len(),
    })
}

/// `counts` with one more pass laid over it: a lit pixel counts one more.
fn add(counts: &[(u64, u8)], plane: &[(u64, bool)]) -> Vec<(u64, u8)> {
    let mut summed: Vec<(u64, u8)> = Vec::with_capacity(counts.len() + plane.len());
    let (mut counts, mut plane) = (counts.iter().copied(), plane.iter().copied());
    let (mut count, mut bit) = (counts.next(), plane.next());
    while let (Some((count_left, base)), Some((bit_left, lit))) = (count, bit) {
        let length = count_left.min(bit_left);
        let value = base + u8::from(lit);
        match summed.last_mut() {
            Some((last, was)) if *was == value => *last += length,
            _ => summed.push((length, value)),
        }
        count = (count_left > length)
            .then_some((count_left - length, base))
            .or_else(|| counts.next());
        bit = (bit_left > length)
            .then_some((bit_left - length, lit))
            .or_else(|| plane.next());
    }
    summed
}

/// The grey a pixel lit in `count` of `levels` passes is shown at.
fn grey(count: u8, levels: u32) -> u8 {
    (u32::from(count) * 256 / levels).saturating_sub(1).min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(pairs: &[(u32, u8)]) -> Vec<Run> {
        pairs
            .iter()
            .map(|&(length, value)| Run { length, value })
            .collect()
    }

    #[test]
    fn one_pass_is_black_and_white() {
        // 3 unlit, 5 lit, 2 unlit over a ten-pixel panel.
        let decoded = decode(&[0x03, 0x85, 0x02], 10, 1).expect("the pass fills the panel");
        assert_eq!(decoded, runs(&[(3, 0), (5, 255), (2, 0)]));
    }

    #[test]
    fn passes_sum_into_grey() {
        // Four passes over eight pixels lighting 8, 6, 4 and 2 of them from the left.
        let data = [0x88, 0x86, 0x02, 0x84, 0x04, 0x82, 0x06];
        let decoded = decode(&data, 8, 4).expect("four passes fill the panel four times");
        assert_eq!(
            decoded,
            runs(&[(2, 255), (2, 191), (2, 127), (2, 63)]),
            "a pixel lit in c of 4 passes is c * 64 - 1"
        );
    }

    #[test]
    fn a_pixel_lit_in_every_pass_is_white_whatever_the_count() {
        for levels in 1..=MAX_LEVELS {
            assert_eq!(
                grey(levels as u8, levels),
                255,
                "{levels} of {levels} passes"
            );
        }
    }

    #[test]
    fn a_run_past_the_panel_is_refused_where_it_is() {
        let refused = decode(&[0x05, 0x86], 10, 1);
        assert!(matches!(
            refused,
            Err(FormatError::MalformedRun { offset: 1 })
        ));
    }

    #[test]
    fn a_layer_short_of_its_passes_is_refused() {
        let refused = decode(&[0x8A], 10, 2);
        assert!(matches!(
            refused,
            Err(FormatError::MalformedRun { offset: 1 })
        ));
    }
}
