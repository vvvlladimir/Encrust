//! What the Chitu containers share whatever their revision or encryption: the layer cipher,
//! the notice, a layer's peel and its area tail. See `docs/formats/chitu.md`.

use std::io;

use core_raster::LayerRuns;
use core_thumbnail::{Rgb, Thumbnail};

use crate::{
    Fields, PREVIEW_SIZES_PX, PrintJob, PrinterTuning, Rle7Layer, SecondStage, stated_distance_mm,
    write_preview,
};

/// The notice CBD Technology puts in every version 4 file, at the length it expects.
///
/// It is a field of the container, not a claim of ours; see `docs/formats/chitu.md`.
pub const CHITU_DISCLAIMER: &str = "Layout and record format for the ctb and cbddlp file types are the copyrighted programs or codes of CBD Technology (China) Inc..The Customer or User shall not in any manner reproduce, distribute, modify, decompile, disassemble, decrypt, extract, reverse engineer, lease, assign, or sublicense the said programs or codes.";
/// The bytes the format gives the notice.
pub const CHITU_DISCLAIMER_BYTES: u32 = 320;

/// The resin type a file states for a resin that names none, as the vendor's slicer does.
pub const CHITU_RESIN_TYPE: &str = "normal";

/// Bytes of the resin block in front of its three strings.
pub const CHITU_RESIN_BYTES: u32 = 40;

/// Colour a preview record is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// Grey levels both `.ctb` containers and `.cxdlp` state an anti-aliased stack in.
pub const CHITU_ANTI_ALIAS_CAP: u16 = 8;

/// One layer's seven-bit runs, and how many pixels they light, as both `.ctb` crates write it.
pub struct ChituLayer {
    runs: Rle7Layer,
    lit_px: u64,
}

impl ChituLayer {
    /// Compresses a layer and counts what it lights.
    pub fn encode(layer: &LayerRuns) -> Self {
        Self {
            runs: Rle7Layer::encode(layer),
            lit_px: layer.lit_px(),
        }
    }

    pub fn runs(&self) -> &Rle7Layer {
        &self.runs
    }

    pub fn lit_px(&self) -> u64 {
        self.lit_px
    }

    /// The layer's runs ciphered as layer `index` under `key`, its definition written by
    /// `definition` in front and its area tail behind, as one block.
    pub fn block(
        &self,
        job: &PrintJob,
        key: u32,
        index: u32,
        definition: impl FnOnce(&mut Fields<'_>) -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        let mut data = self.runs.data().to_vec();
        chitu_layer_crypt(key, index, &mut data);
        let mut bytes = std::io::Cursor::new(Vec::with_capacity(data.len() + 96));
        {
            let mut fields = Fields::new(&mut bytes);
            definition(&mut fields)?;
            fields.bytes(&data)?;
            write_chitu_layer_tail(&mut fields, job, self.lit_px)?;
        }
        Ok(bytes.into_inner())
    }
}

/// What a Chitu container states the whole stack adds up to once every layer is in.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChituTotals {
    /// Seconds every layer spends lit, dark and resting, added up.
    pub layer_time_s: f32,
    /// Bytes of the largest layer's run-length data.
    pub largest_layer_bytes: u32,
    pub layers: u32,
}

impl ChituTotals {
    /// Takes layer `index` of `job`, whose data is `data_bytes` long, into the totals.
    pub fn add(&mut self, job: &PrintJob, index: u32, data_bytes: u32) {
        let material = &job.material;
        let rests_s: f32 = material.waits.rests_s().iter().sum();
        self.layer_time_s +=
            job.exposure_of_layer_s(index) + material.light_off_of_layer_s(index) + rests_s;
        self.largest_layer_bytes = self.largest_layer_bytes.max(data_bytes);
        self.layers += 1;
    }
}

/// The low byte of the slicer info's flag word, the same with grey or without as the
/// vendor's slicer writes it.
pub const CHITU_SLICER_FLAGS: u32 = 0x07;

/// Both preview records, large then small, cut from the job's thumbnail or black, and
/// where each starts.
pub fn write_chitu_previews(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<(u32, u32)> {
    let [large, small] = PREVIEW_SIZES_PX.map(|(width, height)| {
        let image =
            Thumbnail::fitted_or_filled(job.thumbnail.as_ref(), width, height, PREVIEW_BACKGROUND);
        (width, height, image)
    });
    let large = write_preview(fields, large.0, large.1, large.2.pixels())?;
    let small = write_preview(fields, small.0, small.1, small.2.pixels())?;
    Ok((large, small))
}

/// The resin block at `at` and its three strings: the resin's type, its name and the
/// preset's name, each addressed and measured from the block.
pub fn write_chitu_resin(fields: &mut Fields<'_>, job: &PrintJob, at: u32) -> io::Result<()> {
    let material = &job.material;
    let kind = if material.details.kind.is_empty() {
        CHITU_RESIN_TYPE
    } else {
        material.details.kind.as_str()
    };
    let strings = [
        kind.as_bytes(),
        material.name.as_bytes(),
        material.name.as_bytes(),
    ];

    fields.zeros(8)?;
    let mut address = at + CHITU_RESIN_BYTES;
    for text in strings {
        fields.u32_le(address)?;
        fields.u32_le(text.len() as u32)?;
        address += text.len() as u32;
    }
    fields.f32_le(material.density_g_cm3)?;
    fields.u32_le(0)?;
    for text in strings {
        fields.bytes(text)?;
    }
    Ok(())
}

/// The tail behind each layer's data in a version 5 `.ctb`, plain or encrypted: the
/// `lit_px` it lights in mm², then a zero word.
fn write_chitu_layer_tail(fields: &mut Fields<'_>, job: &PrintJob, lit_px: u64) -> io::Result<()> {
    let (pitch_x_mm, pitch_y_mm) = job.printer.display.pixel_pitch_mm();
    fields.f32_le((lit_px as f64 * f64::from(pitch_x_mm) * f64::from(pitch_y_mm)) as f32)?;
    fields.u32_le(0)
}

/// A layer's peel as both `.ctb` containers state it: lift (both stages' whole climb), lift
/// speed, second-stage lift and speed, retract speed, second-stage retract and speed.
pub fn chitu_layer_motion(job: &PrintJob, index: u32) -> [f32; 7] {
    let peel = job.material.peel_of_layer(index);
    [
        peel.lift_mm + peel.second_lift_mm,
        peel.lift_speed_mm_min,
        peel.second_lift_mm,
        peel.second_lift_speed_mm_min,
        peel.retract_speed_mm_min,
        peel.second_retract_mm,
        peel.second_retract_speed_mm_min,
    ]
}

/// The second stage but for its bottom retract, as the six words every Chitu container holds
/// it in: bottom lift and its speed, lift and its speed, retract and its speed.
pub fn chitu_second_stage_words(stage: &SecondStage) -> [f32; 6] {
    [
        stage.bottom_lift_distance_mm,
        stage.bottom_lift_speed_mm_min,
        stage.lift_distance_mm,
        stage.lift_speed_mm_min,
        stage.retract_distance_mm,
        stage.retract_speed_mm_min,
    ]
}

/// A peel as a Chitu container states it: each lift both stages' whole climb, which the
/// retract travels back, the second stage in its six words and its bottom retract apart.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChituPeel {
    pub bottom_lift_total_mm: f32,
    pub lift_total_mm: f32,
    pub second_stage: [f32; 6],
    /// The second stage's bottom retract: distance in mm, then speed in mm/min.
    pub bottom_retract: [f32; 2],
}

impl ChituPeel {
    /// The lifts and retract of the first stage, split off the whole climbs, and the second
    /// stage where one moves; see `docs/decisions/0363-a-peel-has-two-stages.md`.
    pub fn stated(self) -> PrinterTuning {
        let [bl_mm, bl_speed, l_mm, l_speed, r_mm, r_speed] = self.second_stage;
        let [br_mm, br_speed] = self.bottom_retract;
        let stage = SecondStage {
            lift_distance_mm: l_mm,
            lift_speed_mm_min: l_speed,
            retract_distance_mm: r_mm,
            retract_speed_mm_min: r_speed,
            bottom_lift_distance_mm: bl_mm,
            bottom_lift_speed_mm_min: bl_speed,
            bottom_retract_distance_mm: br_mm,
            bottom_retract_speed_mm_min: br_speed,
        };
        // A stage past the whole climb it is part of is no split the file can mean.
        let stage = stage.is_used().then_some(stage).filter(|stage| {
            stage.lift_distance_mm.max(stage.retract_distance_mm) <= self.lift_total_mm
                && stage.bottom_lift_distance_mm <= self.bottom_lift_total_mm
        });
        let split = stage.unwrap_or_default();
        PrinterTuning {
            bottom_lift_distance_mm: stated_distance_mm(
                self.bottom_lift_total_mm - split.bottom_lift_distance_mm,
            ),
            lift_distance_mm: stated_distance_mm(self.lift_total_mm - split.lift_distance_mm),
            retract_distance_mm: stated_distance_mm(self.lift_total_mm - split.retract_distance_mm),
            second_stage: stage,
            ..PrinterTuning::default()
        }
    }
}

/// Applies the `.ctb` layer cipher to one layer's run-length data in place. A key of zero means
/// clear data (ADR 0046); see `docs/formats/chitu.md`.
pub fn chitu_layer_crypt(key: u32, layer_index: u32, data: &mut [u8]) {
    if key == 0 {
        return;
    }
    let init = key.wrapping_mul(0x2D83_CDAC).wrapping_add(0xD8A8_3423);
    let word = layer_index
        .wrapping_mul(0x1E15_30CD)
        .wrapping_add(0xEC3D_47CD)
        .wrapping_mul(init);
    chitu_keystream_xor(data, word, init);
}

/// Exclusive-ors each little-endian four bytes with `word`, then steps it on by `step`; a
/// short last chunk takes the low bytes. The keyed ciphers of the Chitu layouts differ only
/// in the two seeds.
pub fn chitu_keystream_xor(data: &mut [u8], mut word: u32, step: u32) {
    for chunk in data.chunks_mut(4) {
        for (byte, mask) in chunk.iter_mut().zip(word.to_le_bytes()) {
            *byte ^= mask;
        }
        word = word.wrapping_add(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_disclaimer_fits_the_field_the_format_fixes() {
        assert_eq!(
            CHITU_DISCLAIMER.len(),
            CHITU_DISCLAIMER_BYTES as usize,
            "the notice is written whole, not truncated, at 320 bytes"
        );
    }

    #[test]
    fn a_key_of_zero_leaves_the_data_alone() {
        let mut data = [1, 2, 3, 4, 5];
        chitu_layer_crypt(0, 7, &mut data);
        assert_eq!(data, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn deciphering_is_the_same_pass_as_enciphering() {
        let plain: Vec<u8> = (0..64).collect();
        let mut data = plain.clone();
        chitu_layer_crypt(0x0378_32AA, 3, &mut data);
        assert_ne!(data, plain, "the data is actually covered");
        chitu_layer_crypt(0x0378_32AA, 3, &mut data);
        assert_eq!(data, plain);
    }

    #[test]
    fn each_layer_is_covered_by_its_own_keystream() {
        let mut first = vec![0; 16];
        let mut second = vec![0; 16];
        chitu_layer_crypt(0x0378_32AA, 0, &mut first);
        chitu_layer_crypt(0x0378_32AA, 1, &mut second);
        assert_ne!(first, second, "the layer index is part of the key");
    }

    #[test]
    fn the_keystream_word_advances_every_four_bytes() {
        // Zeroed input makes the output the keystream itself, so the first four bytes must
        // be one little-endian word and the fifth must begin another.
        let mut data = vec![0; 8];
        chitu_layer_crypt(1, 0, &mut data);
        let init = 1u32.wrapping_mul(0x2D83_CDAC).wrapping_add(0xD8A8_3423);
        let word = 0xEC3D_47CDu32.wrapping_mul(init);
        assert_eq!(data[..4], word.to_le_bytes());
        assert_eq!(data[4..8], word.wrapping_add(init).to_le_bytes()[..4]);
    }
}
