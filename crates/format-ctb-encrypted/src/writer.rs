use std::io::Cursor;

use core_format::{
    CHITU_DISCLAIMER, CHITU_DISCLAIMER_BYTES, CHITU_RESIN_TYPE, Crc32, Fields, FormatError,
    LayerSink, PREVIEW_SIZES_PX, PrintJob, Rgb, Rle7Layer, SlicedFileWriter, WriteSeek,
    chitu_layer_crypt, chitu_layer_motion, chitu_lit_px, validate, write_chitu_layer_tail,
    write_preview,
};
use core_raster::{LayerRuns, Shading};

use crate::cipher;
use crate::layout::{HEADER_BYTES, LAYER_DEF_BYTES, MAGIC, POINTER_BYTES, RESIN_BYTES, paged};
use crate::settings::{self as at, SETTINGS_BYTES, Settings};

/// The revision written: the one the vendor's own slicer writes, with the resin block.
const VERSION: u32 = 5;

/// The key the layer runs are covered with; a file the machine takes always states one.
const LAYER_KEY: u32 = 0xEFBE_ADDE;

/// What the settings' checksum states, which the signature is derived from.
const CHECKSUM: u64 = 0;

/// Bytes of the signature, two cipher blocks.
const SIGNATURE_BYTES: u32 = 32;

/// The per-layer settings byte of a version 5 `.ctb`, set only where a layer differs from
/// what the settings state.
const PER_LAYER_SETTINGS: u8 = 0x50;

/// Colour a preview record is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// Writes the encrypted `.ctb`. Layout is in `docs/formats/ctb-encrypted.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CtbEncryptedWriter;

impl SlicedFileWriter for CtbEncryptedWriter {
    type Sink<'w> = CtbEncryptedSink<'w>;

    fn extension(&self) -> &'static str {
        "ctb"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;
        // Everything in front of the first layer is kept, to be patched and summed at the end.
        let mut front = Cursor::new(Vec::new());
        let (blocks, pointers) = {
            let mut fields = Fields::new(&mut front);
            fields.zeros(HEADER_BYTES as usize + SETTINGS_BYTES)?;
            let blocks = write_blocks(&mut fields, job)?;
            let pointers = fields.position()?;
            fields.zeros((POINTER_BYTES * u64::from(job.layer_count())) as usize)?;
            (blocks, pointers)
        };
        let front = front.into_inner();
        let mut fields = Fields::new(sink);
        fields.bytes(&front)?;
        Ok(CtbEncryptedSink {
            fields,
            job: job.clone(),
            blocks,
            pointers,
            front,
            definitions: Vec::with_capacity(job.layer_count() as usize),
            totals: Totals::default(),
            layers_crc: Crc32::default(),
        })
    }
}

/// Where each block in front of the layer table went.
#[derive(Debug, Clone, Copy)]
struct Blocks {
    large_preview: u32,
    small_preview: u32,
    machine_name: (u32, u32),
    disclaimer: u32,
    resin: u32,
}

/// What the stack adds up to, which the settings state once the last layer is in.
#[derive(Debug, Clone, Copy, Default)]
struct Totals {
    /// Seconds every layer spends lit, dark and resting, added up.
    layer_time_s: f32,
    largest_layer_bytes: u32,
}

fn write_blocks(fields: &mut Fields<'_>, job: &PrintJob) -> Result<Blocks, FormatError> {
    let [large, small] = PREVIEW_SIZES_PX;
    let large_preview = write_preview(fields, large.0, large.1, &preview(job, large))?;
    let small_preview = write_preview(fields, small.0, small.1, &preview(job, small))?;

    let name = job.printer.machine_name().as_bytes();
    let machine_name = (fields.position()? as u32, name.len() as u32);
    fields.bytes(name)?;
    let disclaimer = fields.position()? as u32;
    fields.text(CHITU_DISCLAIMER, CHITU_DISCLAIMER_BYTES as usize)?;
    let resin = fields.position()? as u32;
    write_resin(fields, job, resin)?;
    Ok(Blocks {
        large_preview,
        small_preview,
        machine_name,
        disclaimer,
        resin,
    })
}

/// The job's thumbnail cut to one record's shape, or black when there is none.
fn preview(job: &PrintJob, (width, height): (u32, u32)) -> Vec<Rgb> {
    match job.thumbnail.as_ref() {
        Some(thumbnail) => thumbnail
            .fitted_to(width, height, PREVIEW_BACKGROUND)
            .pixels()
            .to_vec(),
        None => vec![PREVIEW_BACKGROUND; (width * height) as usize],
    }
}

/// The resin block as the plain `.ctb` lays it: a zero colour, then the address and length
/// of the resin's type, its name and the preset's name, then its density, the strings behind.
fn write_resin(fields: &mut Fields<'_>, job: &PrintJob, at: u32) -> Result<(), FormatError> {
    let resin = &job.material;
    let kind = if resin.details.kind.is_empty() {
        CHITU_RESIN_TYPE
    } else {
        resin.details.kind.as_str()
    };
    let strings = [
        kind.as_bytes(),
        resin.name.as_bytes(),
        resin.name.as_bytes(),
    ];

    fields.zeros(8)?;
    let mut address = at + RESIN_BYTES as u32;
    for text in strings {
        fields.u32_le(address)?;
        fields.u32_le(text.len() as u32)?;
        address += text.len() as u32;
    }
    fields.f32_le(resin.density_g_cm3)?;
    fields.u32_le(0)?;
    for text in strings {
        fields.bytes(text)?;
    }
    Ok(())
}

/// One layer's runs, and how many pixels they light.
pub struct EncryptedLayer {
    runs: Rle7Layer,
    lit_px: u64,
}

/// An encrypted `.ctb` with everything in front of its layers written.
pub struct CtbEncryptedSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    blocks: Blocks,
    pointers: u64,
    /// The header, settings, blocks and pointer table, rewritten once the stack is in.
    front: Vec<u8>,
    /// Where each layer's definition went.
    definitions: Vec<u64>,
    totals: Totals,
    layers_crc: Crc32,
}

impl LayerSink for CtbEncryptedSink<'_> {
    type Encoded = EncryptedLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncryptedLayer {
            runs: Rle7Layer::encode(layer),
            lit_px: chitu_lit_px(layer),
        }
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.definitions.len() as u32;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        let runs = &encoded.runs;
        if (runs.width(), runs.height()) != (self.job.raster.width_px, self.job.raster.height_px) {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width: runs.width(),
                height: runs.height(),
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }
        let at = self.fields.position()?;
        let mut data = runs.data().to_vec();
        chitu_layer_crypt(LAYER_KEY, index, &mut data);
        let mut bytes = Cursor::new(Vec::with_capacity(data.len() + 96));
        {
            let mut layer = Fields::new(&mut bytes);
            write_definition(&mut layer, &self.job, index, at, data.len())?;
            layer.bytes(&data)?;
            write_chitu_layer_tail(&mut layer, &self.job, encoded.lit_px)?;
        }
        let bytes = bytes.into_inner();
        self.layers_crc.update(&bytes);
        self.fields.bytes(&bytes)?;

        let material = &self.job.material;
        let rests: f32 = material.waits.rests_s().iter().sum();
        self.totals.layer_time_s +=
            self.job.exposure_of_layer_s(index) + material.light_off_of_layer_s(index) + rests;
        self.totals.largest_layer_bytes = self.totals.largest_layer_bytes.max(data.len() as u32);
        self.definitions.push(at);
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.definitions.len() as u32;
        if written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written,
            });
        }
        let end_of_layers = self.fields.position()?;
        let signature_at = end_of_layers as u32;
        let signature = cipher::signature(CHECKSUM);
        self.patch_front(signature_at)?;

        // The file ends with the CRC of everything before it, as the vendor's slicer ends it.
        let mut crc = Crc32::default();
        crc.update(&self.front);
        let mut tail = Crc32::default();
        tail.update(&signature);
        let crc = crc.followed_by(self.layers_crc).followed_by(tail);

        self.fields.seek_to(0)?;
        self.fields.bytes(&self.front)?;
        self.fields.seek_to(end_of_layers)?;
        self.fields.bytes(&signature)?;
        self.fields.u32_le(crc.zlib())?;
        self.fields.flush()?;
        Ok(())
    }
}

impl CtbEncryptedSink<'_> {
    /// Lays the pointer table, the enciphered settings and the header over the front.
    fn patch_front(&mut self, signature_at: u32) -> Result<(), FormatError> {
        let mut settings = settings(&self.job, self.blocks, self.pointers as u32, self.totals);
        cipher::encrypt(&mut settings.0, "settings")?;
        let mut front = Cursor::new(std::mem::take(&mut self.front));
        {
            let mut fields = Fields::new(&mut front);
            fields.seek_to(self.pointers)?;
            for &definition in &self.definitions {
                let (page, offset) = paged(definition);
                for word in [offset, page, LAYER_DEF_BYTES as u32, 0] {
                    fields.u32_le(word)?;
                }
            }
            fields.seek_to(HEADER_BYTES)?;
            fields.bytes(&settings.0)?;
            fields.seek_to(0)?;
            write_header(&mut fields, signature_at)?;
        }
        self.front = front.into_inner();
        Ok(())
    }
}

fn write_header(fields: &mut Fields<'_>, signature_at: u32) -> Result<(), FormatError> {
    for word in [
        MAGIC,
        SETTINGS_BYTES as u32,
        HEADER_BYTES as u32,
        0,
        VERSION,
        SIGNATURE_BYTES,
    ] {
        fields.u32_le(word)?;
    }
    fields.u32_le(signature_at)?;
    fields.u32_le(0)?;
    fields.u16_le(1)?;
    fields.u16_le(1)?;
    for word in [0, 42, 0] {
        fields.u32_le(word)?;
    }
    Ok(())
}

/// Layer `index`'s definition, its data following it at `at` plus the definition's size.
fn write_definition(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    index: u32,
    at: u64,
    data_bytes: usize,
) -> Result<(), FormatError> {
    let resin = &job.material;
    let pwm = if resin.is_bottom_layer(index) {
        resin.bottom_light_pwm
    } else {
        resin.light_pwm
    };
    let (page, offset) = paged(at + LAYER_DEF_BYTES);
    fields.u32_le(LAYER_DEF_BYTES as u32)?;
    fields.f32_le(job.layer_z_mm(index))?;
    fields.f32_le(job.exposure_of_layer_s(index))?;
    fields.f32_le(resin.light_off_of_layer_s(index))?;
    for word in [offset, page, data_bytes as u32, 0, 0, 0] {
        fields.u32_le(word)?;
    }
    for value in chitu_layer_motion(job, index) {
        fields.f32_le(value)?;
    }
    for rest_s in resin.waits.rests_s() {
        fields.f32_le(rest_s)?;
    }
    fields.f32_le(f32::from(pwm))?;
    fields.u32_le(0)?;
    Ok(())
}

/// The settings block in the clear: the job's numbers and where `blocks` put everything.
fn settings(job: &PrintJob, blocks: Blocks, pointers: u32, totals: Totals) -> Settings {
    let mut settings = Settings([0; SETTINGS_BYTES]);
    let set = &mut settings;
    set.put_u32(at::CHECKSUM, CHECKSUM as u32);
    set.put_u32(at::LAYER_POINTERS, pointers);
    set.put_f32(at::DISPLAY_MM, job.printer.display.width_mm);
    set.put_f32(at::DISPLAY_MM + 4, job.printer.display.height_mm);
    set.put_f32(at::MACHINE_Z_MM, job.printer.build_volume.z);
    set.put_f32(at::LAYER_HEIGHT_MM, job.nominal_height_mm());
    set.put_f32(at::EXPOSURE_S, job.header_exposure_s());
    set.put_f32(at::BOTTOM_EXPOSURE_S, job.material.bottom_exposure_s);
    set.put_u32(at::BOTTOM_LAYERS, job.material.bottom_layers);
    set.put_u32(at::RESOLUTION_PX, job.raster.width_px);
    set.put_u32(at::RESOLUTION_PX + 4, job.raster.height_px);
    set.put_u32(at::LAYER_COUNT, job.layer_count());
    set.put_u32(at::LARGE_PREVIEW, blocks.large_preview);
    set.put_u32(at::SMALL_PREVIEW, blocks.small_preview);
    set.put_u32(at::PRINT_TIME_S, job.print_time_s());
    let mirrored = job.printer.mirror_x || job.printer.mirror_y;
    set.put_u32(at::PROJECTOR, u32::from(mirrored));
    set.put_f32(at::VOLUME_ML, job.volume_mm3 / 1000.0);
    set.put_f32(at::WEIGHT_G, job.weight_g());
    set.put_f32(at::COST, job.cost().unwrap_or(0.0));
    set.put_u32(at::ONE, 1);
    set.put_u32(at::LAYER_KEY, LAYER_KEY);
    set.put_u32(at::MACHINE_NAME, blocks.machine_name.0);
    set.put_u32(at::MACHINE_NAME + 4, blocks.machine_name.1);
    set.put_u32(at::DISCLAIMER, blocks.disclaimer);
    set.put_u32(at::DISCLAIMER + 4, CHITU_DISCLAIMER_BYTES);
    set.put_u32(at::ONE_AGAIN, 1);
    set.put_u32(at::RESIN, blocks.resin);
    set.put_f32(at::LAYER_TIME_S, totals.layer_time_s);
    set.put_u32(at::LARGEST_LAYER_BYTES, totals.largest_layer_bytes);
    set.put_u32(at::LAYER_COUNT_AGAIN, job.layer_count());
    put_motion(set, job);
    put_slicer_info(set, job);
    settings
}

/// The peel, the light and the rests, which the settings state for the stack as a whole.
fn put_motion(set: &mut Settings, job: &PrintJob) {
    let resin = &job.material;
    let stage = &resin.second_stage;
    set.put_f32(at::LIGHT_OFF_S, resin.light_off_s());
    set.put_f32(at::BOTTOM_LIGHT_OFF_S, resin.bottom_light_off_s());
    set.put_f32(at::BOTTOM_LIFT_MM, resin.bottom_lift_total_mm());
    set.put_f32(at::BOTTOM_LIFT_SPEED, resin.bottom_lift_speed_mm_min);
    set.put_f32(at::LIFT_MM, resin.lift_total_mm());
    set.put_f32(at::LIFT_SPEED, resin.lift_speed_mm_min);
    set.put_f32(at::RETRACT_SPEED, resin.retract_speed_mm_min);
    set.put_f32(at::BOTTOM_RETRACT_SPEED, resin.bottom_retract_speed_mm_min);
    set.put_f32(
        at::BOTTOM_RETRACT_SPEED_2,
        stage.bottom_retract_speed_mm_min,
    );
    set.put_f32(at::BOTTOM_RETRACT_MM_2, stage.bottom_retract_distance_mm);
    for (index, value) in [
        stage.bottom_lift_distance_mm,
        stage.bottom_lift_speed_mm_min,
        stage.lift_distance_mm,
        stage.lift_speed_mm_min,
        stage.retract_distance_mm,
        stage.retract_speed_mm_min,
    ]
    .into_iter()
    .enumerate()
    {
        set.put_f32(at::SECOND_STAGE + 4 * index, value);
    }
    set.put_u16(at::LIGHT_PWM, u16::from(resin.light_pwm));
    set.put_u16(at::BOTTOM_LIGHT_PWM, u16::from(resin.bottom_light_pwm));
    let [before_lift_s, after_lift_s, after_retract_s] = resin.waits.rests_s();
    for (offset, rest) in [
        (at::FIRST_REST_BEFORE_LIFT_S, before_lift_s),
        (at::FIRST_REST_AFTER_RETRACT_S, after_retract_s),
        (at::FIRST_REST_AFTER_LIFT_S, after_lift_s),
        (at::REST_AFTER_RETRACT_S, after_retract_s),
        (at::REST_AFTER_LIFT_S, after_lift_s),
        (at::REST_BEFORE_LIFT_S, before_lift_s),
    ] {
        set.put_f32(offset, rest);
    }
}

/// What the plain `.ctb` keeps in its slicer info and version 4 blocks.
fn put_slicer_info(set: &mut Settings, job: &PrintJob) {
    set.put_u32(at::FLAGS, flags(job));
    set.put_u32(at::MODIFIED_MINUTES, job.created_minutes());
    set.put_u32(
        at::ANTI_ALIAS_LEVEL,
        u32::from(job.raster.anti_alias_level(8)),
    );
    set.put_u32(
        at::TRANSITION_LAYERS,
        u32::from(job.material.transition_layers),
    );
    for offset in at::FOUR {
        set.put_f32(offset, 4.0);
    }
}

/// Grey in the low byte; in the high byte whether the machine is to obey each definition,
/// which the vendor leaves clear for a stack the settings describe, transition ramp included.
fn flags(job: &PrintJob) -> u32 {
    let grey = match job.raster.shading {
        Shading::Binary => 0x07,
        Shading::Coverage => 0x0F,
    };
    let per_layer =
        job.printer.firmware.per_layer_settings && (!job.exposure.is_empty() || !job.is_uniform());
    grey | u32::from(if per_layer { PER_LAYER_SETTINGS } else { 0 }) << 24
}

#[cfg(test)]
mod tests;
