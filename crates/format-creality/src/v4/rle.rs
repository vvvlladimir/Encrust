//! The version 4 codecs whose runs restart on every row. Layout is in
//! `docs/formats/creality.md`.

use core_format::FormatError;
use core_raster::{LayerRuns, Run, push_run};

/// The flag on the byte that opens a run; its low bits are the grey.
const RUN: u8 = 0x80;
/// Bits of a length byte; the length is stored less one, most significant group first.
const GROUP_BITS: u32 = 7;
const GROUP_MASK: u8 = 0x7F;
/// The bits of a paired run's head read as its level, the head stating it twice.
const PAIRED_LEVEL_MASK: u8 = 0x07;

/// Compresses a layer into runs of `grey_bits` that restart on every row.
pub(super) fn encode_rows(layer: &LayerRuns, grey_bits: u32) -> Vec<u8> {
    let shift = 8 - grey_bits;
    let mut data = Vec::new();
    let mut pending = None;
    walk_rows(layer, |value, take, row_ended| {
        pending = Some(join(&mut data, pending, value >> shift, take));
        if row_ended {
            flush(&mut data, pending.take());
        }
    });
    flush(&mut data, pending);
    data
}

/// Compresses a layer into runs of two-pixel units, a unit taking the grey of the run that
/// completes it. An odd pixel at the end of a row is dropped, as the firmware's files drop it.
pub(super) fn encode_paired_rows(layer: &LayerRuns) -> Vec<u8> {
    let mut data = Vec::new();
    let mut pending = None;
    let mut carry = 0;
    walk_rows(layer, |value, take, row_ended| {
        let filled = carry + take;
        if filled >= 2 {
            pending = Some(join(&mut data, pending, paired_head(value), filled / 2));
        }
        carry = filled % 2;
        if row_ended {
            flush(&mut data, pending.take());
            carry = 0;
        }
    });
    flush(&mut data, pending);
    data
}

/// Calls `span` with each piece of a run that stays on one row, and whether it ends that row.
fn walk_rows(layer: &LayerRuns, mut span: impl FnMut(u8, u32, bool)) {
    let width = layer.width();
    let mut column = 0;
    for run in layer.runs() {
        let mut left = run.length;
        while left > 0 {
            let take = left.min(width - column);
            column += take;
            left -= take;
            let row_ended = column == width;
            if row_ended {
                column = 0;
            }
            span(run.value, take, row_ended);
        }
    }
}

/// The top three bits of a grey, stated twice over in the head's six.
fn paired_head(value: u8) -> u8 {
    let level = value >> 5;
    level << 3 | level
}

/// Lengthens the pending run by `take` pixels of `grey`, or writes it and opens a new one.
fn join(data: &mut Vec<u8>, pending: Option<(u8, u32)>, grey: u8, take: u32) -> (u8, u32) {
    match pending {
        Some((value, length)) if value == grey => (value, length + take),
        other => {
            flush(data, other);
            (grey, take)
        }
    }
}

fn flush(data: &mut Vec<u8>, pending: Option<(u8, u32)>) {
    if let Some((value, length)) = pending {
        write_run(data, value, length);
    }
}

/// One run: its grey, then its length less one in seven-bit groups, none for a single pixel.
fn write_run(data: &mut Vec<u8>, grey: u8, length: u32) {
    data.push(RUN | grey);
    let stored = length - 1;
    if stored == 0 {
        return;
    }
    let groups = (u32::BITS - stored.leading_zeros()).div_ceil(GROUP_BITS);
    for group in (0..groups).rev() {
        data.push((stored >> (group * GROUP_BITS)) as u8 & GROUP_MASK);
    }
}

/// Reads the run at `cursor`: the head's low seven bits and the length.
fn read_run(data: &[u8], cursor: &mut usize) -> Result<(u8, u32), FormatError> {
    let start = *cursor;
    let head = data[start];
    if head & RUN == 0 {
        return Err(FormatError::MalformedRun { offset: start });
    }
    *cursor += 1;
    let mut stored: u32 = 0;
    while *cursor < data.len() && data[*cursor] & RUN == 0 {
        stored = stored
            .checked_mul(1 << GROUP_BITS)
            .ok_or(FormatError::MalformedRun { offset: *cursor })?
            | u32::from(data[*cursor]);
        *cursor += 1;
    }
    let length = stored
        .checked_add(1)
        .ok_or(FormatError::MalformedRun { offset: start })?;
    Ok((head & !RUN, length))
}

/// What a layer has covered with the run at `offset` added, refusing one past the panel.
fn cover(covered: u32, length: u32, pixel_count: u32, offset: usize) -> Result<u32, FormatError> {
    covered
        .checked_add(length)
        .filter(|&total| total <= pixel_count)
        .ok_or(FormatError::MalformedRun { offset })
}

/// Expands a layer of runs of `grey_bits` back into eight-bit grey, repeating the top bits
/// into the bottom ones, so the highest level reads as 255.
pub(super) fn decode_rows(
    data: &[u8],
    pixel_count: u32,
    grey_bits: u32,
) -> Result<Vec<Run>, FormatError> {
    let mask = ((1u16 << grey_bits) - 1) as u8;
    let mut runs: Vec<Run> = Vec::new();
    let mut covered = 0u32;
    let mut cursor = 0;
    while cursor < data.len() {
        let start = cursor;
        let (head, length) = read_run(data, &mut cursor)?;
        let grey = head & mask;
        let value = grey << (8 - grey_bits) | grey >> (2 * grey_bits - 8);
        covered = cover(covered, length, pixel_count, start)?;
        push_run(&mut runs, length, value);
    }
    Ok(runs)
}

/// Expands a layer of paired runs back into pixels, a dropped odd pixel reading as dark.
pub(super) fn decode_paired_rows(
    data: &[u8],
    width_px: u32,
    pixel_count: u32,
) -> Result<Vec<Run>, FormatError> {
    let row_units = width_px / 2;
    let odd_px = width_px % 2;
    let mut runs: Vec<Run> = Vec::new();
    let mut covered = 0u32;
    let mut column = 0;
    let mut cursor = 0;
    while cursor < data.len() {
        let start = cursor;
        let (head, units) = read_run(data, &mut cursor)?;
        if units > row_units - column {
            return Err(FormatError::MalformedRun { offset: start });
        }
        let row_ended = column + units == row_units;
        let length = units * 2 + if row_ended { odd_px } else { 0 };
        covered = cover(covered, length, pixel_count, start)?;
        let level = head & PAIRED_LEVEL_MASK;
        push_run(&mut runs, units * 2, level << 5 | level << 2 | level >> 1);
        if row_ended {
            if odd_px != 0 {
                push_run(&mut runs, odd_px, 0);
            }
            column = 0;
        } else {
            column += units;
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

    fn pairs(runs: &[Run]) -> Vec<(u32, u8)> {
        runs.iter().map(|run| (run.length, run.value)).collect()
    }

    #[test]
    fn a_dark_row_of_a_15120_px_panel_is_three_bytes() {
        let encoded = encode_rows(&layer(15120, 1, &[(15120, 0)]), 6);
        assert_eq!(encoded, [0x80, 0x76, 0x0F]);
    }

    #[test]
    fn a_run_restarts_on_every_row() {
        let encoded = encode_rows(&layer(4, 2, &[(8, 255)]), 6);
        assert_eq!(encoded, [0xBF, 0x03, 0xBF, 0x03]);
    }

    #[test]
    fn a_single_pixel_takes_no_length() {
        let encoded = encode_rows(&layer(3, 1, &[(1, 0), (1, 255), (1, 0)]), 6);
        assert_eq!(encoded, [0x80, 0xBF, 0x80]);
    }

    #[test]
    fn a_layer_reads_back_in_six_bit_grey() {
        let source = layer(5, 2, &[(3, 0), (4, 255), (2, 128), (1, 0)]);
        let runs = decode_rows(&encode_rows(&source, 6), 10, 6).expect("well formed");
        assert_eq!(pairs(&runs), [(3, 0), (4, 255), (2, 130), (1, 0)]);
    }

    #[test]
    fn seven_bit_rows_state_the_full_level_as_0x7f() {
        let encoded = encode_rows(&layer(4, 1, &[(4, 255)]), 7);
        assert_eq!(encoded, [0xFF, 0x03]);
        let runs = decode_rows(&encoded, 4, 7).expect("well formed");
        assert_eq!(pairs(&runs), [(4, 255)]);
    }

    #[test]
    fn a_paired_row_counts_in_two_pixel_units() {
        let encoded = encode_paired_rows(&layer(8, 1, &[(4, 0), (4, 255)]));
        assert_eq!(encoded, [0x80, 0x01, 0xBF, 0x01]);
    }

    #[test]
    fn a_pair_split_across_two_runs_takes_the_grey_of_the_second() {
        let encoded = encode_paired_rows(&layer(4, 1, &[(3, 0), (1, 255)]));
        assert_eq!(
            encoded,
            [0x80, 0xBF],
            "one dark pair, then the split one lit"
        );
    }

    #[test]
    fn a_paired_row_states_three_bits_of_grey_twice() {
        let encoded = encode_paired_rows(&layer(2, 1, &[(2, 0b1010_0000)]));
        assert_eq!(encoded, [RUN | 0b101_101]);
    }

    #[test]
    fn paired_rows_restart_on_every_row_and_read_back_doubled() {
        let source = layer(4, 2, &[(8, 255)]);
        let encoded = encode_paired_rows(&source);
        assert_eq!(encoded, [0xBF, 0x01, 0xBF, 0x01]);
        let runs = decode_paired_rows(&encoded, 4, 8).expect("well formed");
        assert_eq!(pairs(&runs), [(8, 255)]);
    }

    #[test]
    fn an_odd_pixel_dropped_from_a_paired_row_reads_back_dark() {
        let encoded = encode_paired_rows(&layer(3, 1, &[(3, 255)]));
        let runs = decode_paired_rows(&encoded, 3, 3).expect("well formed");
        assert_eq!(pairs(&runs), [(2, 255), (1, 0)]);
    }

    #[test]
    fn a_paired_run_past_its_row_is_refused() {
        assert!(matches!(
            decode_paired_rows(&[0xBF, 0x02], 4, 8),
            Err(FormatError::MalformedRun { offset: 0 })
        ));
    }

    #[test]
    fn paired_rows_past_the_panel_are_refused() {
        assert!(matches!(
            decode_paired_rows(&[0xBF, 0x01, 0xBF, 0x01], 4, 6),
            Err(FormatError::MalformedRun { offset: 2 })
        ));
    }

    #[test]
    fn a_length_byte_with_no_run_before_it_is_refused() {
        assert!(matches!(
            decode_rows(&[0x05], 10, 6),
            Err(FormatError::MalformedRun { offset: 0 })
        ));
    }

    #[test]
    fn a_length_that_fills_every_bit_is_refused() {
        assert!(matches!(
            decode_rows(&[0x80, 0x0F, 0x7F, 0x7F, 0x7F, 0x7F], u32::MAX, 6),
            Err(FormatError::MalformedRun { offset: 0 })
        ));
    }

    #[test]
    fn runs_past_the_panel_are_refused() {
        assert!(matches!(
            decode_rows(&[0x80, 0x03, 0xBF, 0x03], 6, 6),
            Err(FormatError::MalformedRun { offset: 2 })
        ));
    }
}
