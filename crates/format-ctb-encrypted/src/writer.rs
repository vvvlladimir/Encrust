use core_format::{
    CHITU_DISCLAIMER, CHITU_DISCLAIMER_BYTES, Fields, FormatError, LayerSink, PREVIEW_SIZES_PX,
    PrintJob, Rgb, Rle7Layer, SlicedFileWriter, WriteSeek, chitu_layer_crypt, validate,
    write_preview,
};
use core_raster::LayerRuns;

use crate::cipher;
use crate::layout::{HEADER_BYTES, LAYER_DEF_BYTES, MAGIC, POINTER_BYTES, RESIN_BYTES, paged};
use crate::settings::{self as at, SETTINGS_BYTES, Settings};

/// The revision written: the one the vendor's own slicer writes, with the resin block.
const VERSION: u32 = 5;

/// The key the layer runs are covered with; a file the machine takes always states one.
const LAYER_KEY: u32 = 0xEFBE_ADDE;

/// What the settings' checksum states, which the signature is derived from.
const CHECKSUM: u64 = 0;

/// Words of unknown meaning in front of and behind the signature, as files machines take
/// carry them; see `docs/formats/ctb-encrypted.md`.
const BEFORE_SIGNATURE: [u32; 2] = [0x4220_C0FA, 0];
const AFTER_SIGNATURE: u32 = 0x6D42_A8B3;

/// The per-layer settings byte that lets the machine obey each layer's definition.
const PER_LAYER_SETTINGS: u8 = 0x40;

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
        let mut fields = Fields::new(sink);
        // The header and the settings are written last, over this space.
        fields.zeros(HEADER_BYTES as usize + SETTINGS_BYTES)?;
        let blocks = write_blocks(&mut fields, job)?;
        let pointers = fields.position()?;
        fields.zeros((POINTER_BYTES * u64::from(job.layer_count())) as usize)?;
        Ok(CtbEncryptedSink {
            fields,
            job: job.clone(),
            blocks,
            pointers,
            definitions: Vec::with_capacity(job.layer_count() as usize),
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

/// The resin block: its colour, then the address and length of its type, its name and the
/// machine's, then its density, with the three strings behind it.
fn write_resin(fields: &mut Fields<'_>, job: &PrintJob, at: u32) -> Result<(), FormatError> {
    let resin = &job.material;
    let kind = resin.details.kind.as_bytes();
    let name = resin.name.as_bytes();
    let machine = job.printer.machine_name().as_bytes();
    let kind_at = at + RESIN_BYTES as u32;
    let name_at = kind_at + kind.len() as u32;
    let machine_at = name_at + name.len() as u32;

    let [red, green, blue] = resin.details.color;
    fields.u32_le(0)?;
    fields.bytes(&[blue, green, red, 0])?;
    for (address, text) in [(kind_at, kind), (name_at, name), (machine_at, machine)] {
        fields.u32_le(address)?;
        fields.u32_le(text.len() as u32)?;
    }
    fields.f32_le(resin.density_g_cm3)?;
    fields.u32_le(0)?;
    fields.bytes(kind)?;
    fields.bytes(name)?;
    fields.bytes(machine)?;
    Ok(())
}

/// An encrypted `.ctb` with everything in front of its layers written.
pub struct CtbEncryptedSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    blocks: Blocks,
    pointers: u64,
    /// Where each layer's definition went.
    definitions: Vec<u64>,
}

impl LayerSink for CtbEncryptedSink<'_> {
    type Encoded = Rle7Layer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        Rle7Layer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.definitions.len() as u32;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        if (encoded.width(), encoded.height())
            != (self.job.raster.width_px, self.job.raster.height_px)
        {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width: encoded.width(),
                height: encoded.height(),
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }
        let at = self.fields.position()?;
        let mut data = encoded.data().to_vec();
        chitu_layer_crypt(LAYER_KEY, index, &mut data);
        write_definition(&mut self.fields, &self.job, index, at, data.len())?;
        self.fields.bytes(&data)?;
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
        for word in BEFORE_SIGNATURE {
            self.fields.u32_le(word)?;
        }
        let signature_at = self.fields.position()? as u32;
        self.fields.bytes(&cipher::signature(CHECKSUM))?;
        self.fields.u32_le(AFTER_SIGNATURE)?;

        self.fields.seek_to(self.pointers)?;
        for &definition in &self.definitions {
            let (page, offset) = paged(definition);
            self.fields.u32_le(offset)?;
            self.fields.u32_le(page)?;
            self.fields.u32_le(LAYER_DEF_BYTES as u32)?;
            self.fields.u32_le(0)?;
        }
        let mut settings = settings(&self.job, self.blocks, self.pointers as u32);
        cipher::encrypt(&mut settings.0, "settings")?;
        self.fields.seek_to(HEADER_BYTES)?;
        self.fields.bytes(&settings.0)?;
        self.fields.seek_to(0)?;
        write_header(&mut self.fields, signature_at)?;
        self.fields.flush()?;
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
        32,
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
    let bottom = resin.is_bottom_layer(index);
    let (lift_mm, lift_speed, retract_speed, pwm) = if bottom {
        let speeds = (
            resin.bottom_lift_speed_mm_min,
            resin.bottom_retract_speed_mm_min,
        );
        (
            resin.bottom_lift_distance_mm,
            speeds.0,
            speeds.1,
            resin.bottom_light_pwm,
        )
    } else {
        let speeds = (resin.lift_speed_mm_min, resin.retract_speed_mm_min);
        (resin.lift_distance_mm, speeds.0, speeds.1, resin.light_pwm)
    };
    let (page, offset) = paged(at + LAYER_DEF_BYTES);
    fields.u32_le(LAYER_DEF_BYTES as u32)?;
    fields.f32_le(job.layer_z_mm(index))?;
    fields.f32_le(job.exposure_of_layer_s(index))?;
    fields.f32_le(resin.light_off_s())?;
    for word in [offset, page, data_bytes as u32, 0, 0, 0] {
        fields.u32_le(word)?;
    }
    let [before_lift_s, after_lift_s, after_retract_s] = resin.waits.rests_s();
    for value in [lift_mm, lift_speed, 0.0, 0.0, retract_speed, 0.0, 0.0] {
        fields.f32_le(value)?;
    }
    for value in [before_lift_s, after_lift_s, after_retract_s, f32::from(pwm)] {
        fields.f32_le(value)?;
    }
    fields.u32_le(0)?;
    Ok(())
}

/// The settings block in the clear: the job's numbers and where `blocks` put everything.
fn settings(job: &PrintJob, blocks: Blocks, pointers: u32) -> Settings {
    let mut settings = Settings([0; SETTINGS_BYTES]);
    let set = &mut settings;
    set.put_u32(at::CHECKSUM, CHECKSUM as u32);
    set.put_u32(at::LAYER_POINTERS, pointers);
    set.put_f32(at::DISPLAY_MM, job.printer.display.width_mm);
    set.put_f32(at::DISPLAY_MM + 4, job.printer.display.height_mm);
    set.put_f32(at::MACHINE_Z_MM, job.printer.build_volume.z);
    let last = job.layer_count().saturating_sub(1);
    set.put_f32(at::TOTAL_HEIGHT_MM, job.layer_z_mm(last));
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
    set.put_u32(at::ONE, 1);
    set.put_u32(at::LAYER_KEY, LAYER_KEY);
    set.put_u32(at::MACHINE_NAME, blocks.machine_name.0);
    set.put_u32(at::MACHINE_NAME + 4, blocks.machine_name.1);
    set.put_u32(at::DISCLAIMER, blocks.disclaimer);
    set.put_u32(at::DISCLAIMER + 4, CHITU_DISCLAIMER_BYTES);
    set.put_u32(at::RESIN, blocks.resin);
    set.put_u32(at::LAST_LAYER, last);
    put_motion(set, job);
    put_slicer_info(set, job);
    settings
}

/// The peel, the light and the rests, which the settings state for the stack as a whole.
fn put_motion(set: &mut Settings, job: &PrintJob) {
    let resin = &job.material;
    set.put_f32(at::LIGHT_OFF_S, resin.light_off_s());
    set.put_f32(at::BOTTOM_LIGHT_OFF_S, resin.light_off_s());
    set.put_f32(at::BOTTOM_LIFT_MM, resin.bottom_lift_distance_mm);
    set.put_f32(at::BOTTOM_LIFT_SPEED, resin.bottom_lift_speed_mm_min);
    set.put_f32(at::LIFT_MM, resin.lift_distance_mm);
    set.put_f32(at::LIFT_SPEED, resin.lift_speed_mm_min);
    set.put_f32(at::RETRACT_SPEED, resin.retract_speed_mm_min);
    set.put_f32(at::BOTTOM_RETRACT_SPEED, resin.bottom_retract_speed_mm_min);
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
    set.put_u8(at::ANTI_ALIAS_FLAG, 0x0F);
    set.put_u8(at::PER_LAYER_SETTINGS, PER_LAYER_SETTINGS);
    set.put_u32(at::MODIFIED_MINUTES, job.created_minutes());
    set.put_u32(at::ANTI_ALIAS_LEVEL, 1);
    set.put_u32(
        at::TRANSITION_LAYERS,
        u32::from(job.material.transition_layers),
    );
    for offset in at::FOUR {
        set.put_f32(offset, 4.0);
    }
    set.put_u32(at::FIVE, 5);
}
