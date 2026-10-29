use core_format::{FormatError, LayerEntry, ReadSeek, Reads};

use crate::header::HEADER_SIZE;
use crate::layer::DEFINITION_BYTES;

/// Bytes a layer takes at the least: its two table entries, its definition and two masks
/// of a magic byte, a checksum and a delimiter around no runs at all.
const LEAST_LAYER_BYTES: u64 = 8 + 16 + DEFINITION_BYTES as u64 + 2 * 4;

/// The magic byte in front of a mask's runs, the checksum and delimiter behind them.
const MASK_FRAME_BYTES: u32 = 4;

/// Reads the definition and mask tables of a version 5.1 file: each layer's left half as
/// the entry `SlicedFile` lists, and its right half beside it.
pub(super) fn walk_tables<S: ReadSeek>(
    reads: &mut Reads<S>,
    layer_count: u32,
) -> Result<(Vec<LayerEntry>, Vec<LayerEntry>), FormatError> {
    let claimed = reads.claim("layers", u64::from(layer_count), LEAST_LAYER_BYTES)?;
    reads.seek_to(u64::from(HEADER_SIZE) + 1)?;
    let definitions = u64::from(reads.u32_le()?) + 1;
    let masks = u64::from(reads.u32_le()?) + 1;

    let (mut left, mut right) = (Vec::with_capacity(claimed), Vec::with_capacity(claimed));
    for index in 0..u64::from(layer_count) {
        reads.seek_to(definitions + 8 * index)?;
        let definition = u64::from(reads.u32_le()?);
        reads.seek_to(definition + 6)?;
        let (z_mm, exposure_s) = (reads.f32_le()?, reads.f32_le()?);

        reads.seek_to(masks + 16 * index)?;
        for half in [&mut left, &mut right] {
            let at = reads.u32_le()?;
            let size = reads.u32_le()?;
            let runs = size
                .checked_sub(MASK_FRAME_BYTES)
                .ok_or(FormatError::MalformedRun {
                    offset: at as usize,
                })?;
            half.push(LayerEntry {
                z_mm,
                exposure_s,
                offset: u64::from(at) + 1,
                size: runs,
            });
        }
    }
    Ok((left, right))
}
