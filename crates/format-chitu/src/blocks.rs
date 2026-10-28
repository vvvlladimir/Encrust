use std::io;

use core_format::{
    CHITU_DISCLAIMER, CHITU_DISCLAIMER_BYTES, CHITU_RESIN_TYPE, Fields, PREVIEW_SIZES_PX, PrintJob,
    Rgb, write_preview,
};

use crate::writer::CtbVersion;

pub(crate) const HEADER_BYTES: u64 = 112;
const PRINT_PARAMETERS_BYTES: u32 = 60;
pub(crate) const SLICER_INFO_BYTES: u32 = 76;
const PRINT_PARAMETERS_V4_BYTES: u32 = 464;
const RESIN_PARAMETERS_BYTES: u32 = 40;

/// Colour a preview record is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// What the header says about the container it heads, which is all that separates a
/// `.ctb` from a `.cbddlp`. See `docs/formats/chitu.md`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Head {
    pub magic: u32,
    pub version: u32,
    /// Bilevel passes each layer is written in; one means the layers carry grey.
    pub grey_passes: u32,
    /// Bytes of the slicer info block, or zero when the container has none.
    pub slicer_info_bytes: u32,
    /// The layer cipher's key, zero for data in the clear.
    pub encryption_key: u32,
    /// Whether the header states the print's height, which the vendor leaves zero in a `.ctb`.
    pub states_height: bool,
}

/// Where each block the header points at ended up.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Offsets {
    pub large_preview: u32,
    pub small_preview: u32,
    pub print_parameters: u32,
    pub slicer_info: u32,
    pub v4_parameters: u32,
    pub machine_name: u32,
    pub disclaimer: u32,
    pub resin_parameters: u32,
    pub layer_table: u32,
}

/// What a `.ctb` states about the whole stack once every layer is in.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Totals {
    /// Seconds every layer spends lit, dark and resting, added up.
    pub layer_time_s: f32,
    /// Bytes of the largest layer's run-length data.
    pub largest_layer_bytes: u32,
    pub layers: u32,
}

/// Writes everything between the header and the layer table, in the order the vendor's
/// slicer lays it down: the fixed blocks, the previews, then the strings they address.
///
/// The header is written last, over the space this leaves at the front of the file,
/// because half its fields are offsets into what follows.
pub(crate) fn write_body(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: CtbVersion,
) -> io::Result<Offsets> {
    let mut offsets = Offsets::default();
    fields.seek_to(HEADER_BYTES)?;
    offsets.print_parameters = fields.position()? as u32;
    write_print_parameters(fields, job)?;
    offsets.slicer_info = fields.position()? as u32;
    fields.zeros(SLICER_INFO_BYTES as usize)?;
    offsets.v4_parameters = fields.position()? as u32;
    fields.zeros(PRINT_PARAMETERS_V4_BYTES as usize)?;

    (offsets.large_preview, offsets.small_preview) = write_previews(fields, job)?;

    offsets.machine_name = fields.position()? as u32;
    fields.bytes(job.printer.machine_name().as_bytes())?;
    offsets.disclaimer = fields.position()? as u32;
    fields.text(CHITU_DISCLAIMER, CHITU_DISCLAIMER_BYTES as usize)?;
    if version == CtbVersion::V5 {
        offsets.resin_parameters = fields.position()? as u32;
        write_resin_parameters(fields, job, offsets.resin_parameters)?;
    }
    offsets.layer_table = fields.position()? as u32;

    fields.seek_to(u64::from(offsets.slicer_info))?;
    write_slicer_info(fields, job, version, offsets)?;
    rewrite_v4_parameters(fields, job, offsets, Totals::default())?;
    fields.seek_to(u64::from(offsets.layer_table))?;
    Ok(offsets)
}

/// Writes everything between the header and the layer table of a `.cbddlp`, which is
/// where the older container stops: it carries no slicer info and no version 4 block.
pub(crate) fn write_cbddlp_body(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<Offsets> {
    let mut offsets = Offsets::default();
    fields.seek_to(HEADER_BYTES)?;
    (offsets.large_preview, offsets.small_preview) = write_previews(fields, job)?;
    offsets.print_parameters = fields.position()? as u32;
    write_print_parameters(fields, job)?;
    offsets.layer_table = fields.position()? as u32;
    Ok(offsets)
}

/// Both preview records, large then small, and where each starts.
fn write_previews(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<(u32, u32)> {
    let [(large_w, large_h), (small_w, small_h)] = PREVIEW_SIZES_PX;
    let large = write_preview(
        fields,
        large_w,
        large_h,
        &preview_pixels(job, (large_w, large_h)),
    )?;
    let small = write_preview(
        fields,
        small_w,
        small_h,
        &preview_pixels(job, (small_w, small_h)),
    )?;
    Ok((large, small))
}

/// The job's thumbnail cut to one record's shape, or black when there is none.
pub(crate) fn preview_pixels(job: &PrintJob, (width, height): (u32, u32)) -> Vec<Rgb> {
    match job.thumbnail.as_ref() {
        Some(thumbnail) => thumbnail
            .fitted_to(width, height, PREVIEW_BACKGROUND)
            .pixels()
            .to_vec(),
        None => vec![PREVIEW_BACKGROUND; (width * height) as usize],
    }
}

pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    head: Head,
    offsets: Offsets,
) -> io::Result<()> {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);

    fields.u32_le(head.magic)?;
    fields.u32_le(head.version)?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;
    fields.zeros(8)?;
    fields.f32_le(if head.states_height {
        job.height_mm()
    } else {
        0.0
    })?;
    fields.f32_le(job.nominal_height_mm())?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.bottom_exposure_s)?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(raster.width_px)?;
    fields.u32_le(raster.height_px)?;
    fields.u32_le(offsets.large_preview)?;
    fields.u32_le(offsets.layer_table)?;
    fields.u32_le(job.layer_count())?;
    fields.u32_le(offsets.small_preview)?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(u32::from(job.printer.mirror_x || job.printer.mirror_y))?;
    fields.u32_le(offsets.print_parameters)?;
    fields.u32_le(PRINT_PARAMETERS_BYTES)?;

    fields.u32_le(head.grey_passes)?;
    fields.u16_le(u16::from(material.light_pwm))?;
    fields.u16_le(u16::from(material.bottom_light_pwm))?;

    fields.u32_le(head.encryption_key)?;
    fields.u32_le(offsets.slicer_info)?;
    fields.u32_le(head.slicer_info_bytes)
}

/// Lays the print parameters down again, for the totals only known once the stack is
/// cut; see `docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md`.
pub(crate) fn rewrite_print_parameters(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    at: u32,
) -> io::Result<()> {
    fields.seek_to(u64::from(at))?;
    write_print_parameters(fields, job)
}

/// The lifts are both stages' whole climb at the first stage's speed; the second stage is
/// in the slicer info.
fn write_print_parameters(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;

    fields.f32_le(material.bottom_lift_total_mm())?;
    fields.f32_le(material.bottom_lift_speed_mm_min)?;
    fields.f32_le(material.lift_total_mm())?;
    fields.f32_le(material.lift_speed_mm_min)?;
    fields.f32_le(material.retract_speed_mm_min)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(job.cost().unwrap_or(0.0))?;
    fields.f32_le(material.bottom_light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    // A word the vendor's slicer sets to one in every file.
    fields.u32_le(0)?;
    fields.u32_le(1)?;
    fields.zeros(8)
}

fn write_slicer_info(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: CtbVersion,
    offsets: Offsets,
) -> io::Result<()> {
    let material = &job.material;
    let [_, after_lift_s, after_retract_s] = material.waits.rests_s();

    write_second_stage(fields, job)?;
    fields.f32_le(after_lift_s)?;
    fields.u32_le(offsets.machine_name)?;
    fields.u32_le(job.printer.machine_name().len() as u32)?;
    fields.u32_le(version.flags(job))?;
    fields.u32_le(job.created_minutes())?;
    fields.u32_le(u32::from(job.raster.anti_alias_level(8)))?;
    // The vendor's slicer states no software version here.
    fields.u32_le(0)?;
    fields.f32_le(after_retract_s)?;
    fields.f32_le(after_lift_s)?;
    fields.u32_le(u32::from(material.transition_layers))?;
    fields.u32_le(offsets.v4_parameters)?;
    fields.zeros(8)
}

/// Lays the version 4 block down at its address, with what the stack added up to.
pub(crate) fn rewrite_v4_parameters(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    offsets: Offsets,
    totals: Totals,
) -> io::Result<()> {
    fields.seek_to(u64::from(offsets.v4_parameters))?;
    write_v4_motion(fields, job)?;
    fields.f32_le(totals.layer_time_s)?;
    fields.u32_le(totals.largest_layer_bytes)?;
    fields.u32_le(0)?;
    fields.u32_le(totals.layers)?;
    // The build area's offsets from each edge, which no profile of ours states.
    fields.zeros(16)?;
    fields.u32_le(offsets.disclaimer)?;
    fields.u32_le(CHITU_DISCLAIMER_BYTES)?;
    fields.u32_le(offsets.resin_parameters)?;
    fields.zeros(380)
}

/// The second lift and retract stage but for the bottom retract, which the v4 parameters
/// hold; `.fdg` and `.phz` copy these six words into their header.
pub(crate) fn write_second_stage(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let stage = &job.material.second_stage;
    fields.f32_le(stage.bottom_lift_distance_mm)?;
    fields.f32_le(stage.bottom_lift_speed_mm_min)?;
    fields.f32_le(stage.lift_distance_mm)?;
    fields.f32_le(stage.lift_speed_mm_min)?;
    fields.f32_le(stage.retract_distance_mm)?;
    fields.f32_le(stage.retract_speed_mm_min)
}

/// The first 40 bytes of the v4 parameters: the bottom retract's two speeds, the rests
/// around a lift, the second stage's bottom retract, and the words between. `.fdg` and
/// `.phz` copy them into their header at 0xD8.
pub(crate) fn write_v4_motion(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    let stage = &material.second_stage;
    fields.f32_le(material.bottom_retract_speed_mm_min)?;
    fields.f32_le(stage.bottom_retract_speed_mm_min)?;
    fields.u32_le(0)?;

    // Two fields the machine checks for the literal 4.0.
    fields.f32_le(4.0)?;
    fields.u32_le(0)?;
    fields.f32_le(4.0)?;

    let [before_lift_s, after_lift_s, after_retract_s] = material.waits.rests_s();
    fields.f32_le(after_retract_s)?;
    fields.f32_le(after_lift_s)?;
    fields.f32_le(before_lift_s)?;
    fields.f32_le(stage.bottom_retract_distance_mm)
}

/// The resin block and its three strings: the resin's type, its name and the preset's name,
/// each addressed and measured from the block.
fn write_resin_parameters(fields: &mut Fields<'_>, job: &PrintJob, at: u32) -> io::Result<()> {
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

    fields.u32_le(0)?;
    fields.bytes(&[0, 0, 0, 0])?;
    let mut address = at + RESIN_PARAMETERS_BYTES;
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
