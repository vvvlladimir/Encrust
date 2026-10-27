use std::io;

use core_format::{Fields, PrintJob};

/// Bytes of one record in the layer table.
pub(crate) const LAYER_DEF_BYTES: u64 = 36;

/// Bytes of the block `.ctb` puts in front of a layer's run-length data: the record
/// again, then the motion this layer is printed with. Also what its table size field
/// states, because the field counts the record and that block together.
pub(crate) const LAYER_DEF_EX_BYTES: u32 = 84;

/// Bytes the vendor's slicer puts behind a `.ctb` layer's data: its lit area, then a word.
pub(crate) const LAYER_TAIL_BYTES: u32 = 8;

/// One row of the layer table, filled in as its layer is written.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LayerDef {
    pub z_mm: f32,
    pub exposure_s: f32,
    pub light_off_delay_s: f32,
    pub data_address: u32,
    pub data_size: u32,
}

impl LayerDef {
    /// `table_size` is the record alone for a container with no extended block, and the
    /// record plus that block for one that has it.
    pub(crate) fn write(&self, fields: &mut Fields<'_>, table_size: u32) -> io::Result<()> {
        fields.f32_le(self.z_mm)?;
        fields.f32_le(self.exposure_s)?;
        fields.f32_le(self.light_off_delay_s)?;
        fields.u32_le(self.data_address)?;
        fields.u32_le(self.data_size)?;

        // Page number splits offsets past 4 GB. Nothing this project writes gets there,
        // and a file that did would need the address rebased against the page.
        fields.u32_le(0)?;
        fields.u32_le(table_size)?;
        fields.zeros(8)
    }
}

/// Writes the block that sits in front of a layer's run-length data.
///
/// It repeats the table record and then carries the motion for this layer alone, which is
/// what lets exposure and lift change over the height of a print without reslicing.
pub(crate) fn write_extended(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    index: u32,
    def: &LayerDef,
) -> io::Result<()> {
    let material = &job.material;
    let stage = &material.second_stage;
    let bottom = material.is_bottom_layer(index);
    // The first lift field is both stages' whole climb, as the vendor's slicer states it.
    let (lift, lift_speed, lift_2, lift_speed_2) = if bottom {
        (
            material.bottom_lift_total_mm(),
            material.bottom_lift_speed_mm_min,
            stage.bottom_lift_distance_mm,
            stage.bottom_lift_speed_mm_min,
        )
    } else {
        (
            material.lift_total_mm(),
            material.lift_speed_mm_min,
            stage.lift_distance_mm,
            stage.lift_speed_mm_min,
        )
    };
    let (retract_speed, retract_2, retract_speed_2) = if bottom {
        (
            material.bottom_retract_speed_mm_min,
            stage.bottom_retract_distance_mm,
            stage.bottom_retract_speed_mm_min,
        )
    } else {
        (
            material.retract_speed_mm_min,
            stage.retract_distance_mm,
            stage.retract_speed_mm_min,
        )
    };
    let light_pwm = if bottom {
        material.bottom_light_pwm
    } else {
        material.light_pwm
    };

    def.write(fields, LAYER_DEF_EX_BYTES)?;
    fields.u32_le(LAYER_DEF_EX_BYTES + def.data_size + LAYER_TAIL_BYTES)?;
    for value in [
        lift,
        lift_speed,
        lift_2,
        lift_speed_2,
        retract_speed,
        retract_2,
        retract_speed_2,
    ] {
        fields.f32_le(value)?;
    }
    for rest_s in material.waits.rests_s() {
        fields.f32_le(rest_s)?;
    }
    fields.f32_le(f32::from(light_pwm))
}

/// What follows a layer's data: the area it lights, mm², and a word of zeros.
pub(crate) fn write_tail(fields: &mut Fields<'_>, job: &PrintJob, lit_px: u64) -> io::Result<()> {
    let (pitch_x_mm, pitch_y_mm) = job.printer.display.pixel_pitch_mm();
    fields.f32_le((lit_px as f64 * f64::from(pitch_x_mm) * f64::from(pitch_y_mm)) as f32)?;
    fields.u32_le(0)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    #[test]
    fn a_layer_carries_the_resins_rests() {
        let mut job = sample_job(10);
        job.material.waits = printer_profiles::Waits {
            mode: printer_profiles::WaitMode::Rest,
            before_lift_s: 1.5,
            after_lift_s: 0.5,
            after_retract_s: 2.0,
        };
        let def = LayerDef {
            z_mm: 0.05,
            exposure_s: 2.0,
            light_off_delay_s: 0.0,
            data_address: 0,
            data_size: 0,
        };
        let mut buffer = Cursor::new(Vec::new());
        write_extended(&mut Fields::new(&mut buffer), &job, 5, &def).expect("in memory");
        let bytes = buffer.into_inner();
        let rests: Vec<f32> = (0..3)
            .map(|index| {
                let at = 0x44 + 4 * index;
                f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
            })
            .collect();
        assert_eq!(
            rests,
            [1.5, 0.5, 2.0],
            "before lift, after lift, after retract"
        );
        assert_eq!(bytes.len(), LAYER_DEF_EX_BYTES as usize);
    }
}
