use std::io;

use crate::header::DELIMITER;
use crate::rle::EncodedLayer;
use core_format::Fields;
use core_format::PrintJob;

use crate::version::GooVersion;

/// Marks the start of a layer's run-length data; the checksum does not cover it.
pub(crate) const LAYER_MAGIC: u8 = 0x55;

/// Bytes of a layer's definition with the delimiter that closes it.
pub(crate) const DEFINITION_BYTES: u32 = 66;

pub(crate) fn write(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    index: u32,
    encoded: &EncodedLayer,
) -> io::Result<()> {
    write_definition(fields, job, index, GooVersion::V3)?;

    fields.u32_be(encoded.data_size())?;
    fields.u8(LAYER_MAGIC)?;
    fields.bytes(encoded.data())?;
    fields.u8(encoded.checksum())?;
    fields.bytes(&DELIMITER)
}

/// A layer's definition and the delimiter behind it, in `version`'s byte order.
pub(crate) fn write_definition(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    index: u32,
    version: GooVersion,
) -> io::Result<()> {
    let (order, material) = (version.order(), &job.material);
    let bottom = material.is_bottom_layer(index);

    // Pause position, then the layer position. A layer that pauses stops where it is, as a
    // vendor file carries it; the machine's Z travel here moves the plate off the print.
    order.put_u16(fields, 0)?;
    order.put_f32(fields, job.layer_z_mm(index))?;
    order.put_f32(fields, job.layer_z_mm(index))?;
    order.put_f32(fields, job.exposure_of_layer_s(index))?;
    order.put_f32(fields, material.light_off_of_layer_s(index))?;
    for rest_s in material.waits.rests_s() {
        order.put_f32(fields, rest_s)?;
    }

    let peel = material.peel(bottom);
    let motion = [
        peel.lift_mm,
        peel.lift_speed_mm_min,
        peel.second_lift_mm,
        peel.second_lift_speed_mm_min,
        version.retract_mm(peel.retract_mm, job.layer_height_mm(index)),
        peel.retract_speed_mm_min,
        peel.second_retract_mm,
        peel.second_retract_speed_mm_min,
    ];
    for value in motion {
        order.put_f32(fields, value)?;
    }

    let pwm = if bottom {
        material.bottom_light_pwm
    } else {
        material.light_pwm
    };
    order.put_u16(fields, u16::from(pwm))?;
    fields.bytes(&DELIMITER)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use core_raster::LayerRuns;

    use super::*;
    use crate::fixtures::sample_job;

    fn layer_of(index: u32) -> Vec<u8> {
        let job = sample_job(10);
        let encoded = EncodedLayer::encode(&LayerRuns::builder(8, 4).finish());
        let mut buffer = Cursor::new(Vec::new());
        let mut fields = Fields::new(&mut buffer);
        write(&mut fields, &job, index, &encoded).expect("in-memory write");
        buffer.into_inner()
    }

    fn float_at(bytes: &[u8], offset: usize) -> f32 {
        f32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }

    #[test]
    fn the_layer_z_is_the_top_of_the_layer() {
        let layer = layer_of(0);
        // Pause flag, then pause position, then the layer position.
        assert!((float_at(&layer, 6) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn a_layer_pauses_where_it_stands_rather_than_at_the_top_of_the_travel() {
        let layer = layer_of(3);
        let pause_z = float_at(&layer, 2);
        assert!(
            (pause_z - float_at(&layer, 6)).abs() < 1e-6,
            "a pause position anywhere else moves the plate off the print, got {pause_z}"
        );
    }

    #[test]
    fn a_bottom_layer_takes_the_bottom_exposure() {
        assert!((float_at(&layer_of(0), 10) - 30.0).abs() < 1e-6);
        assert!((float_at(&layer_of(6), 10) - 2.5).abs() < 1e-6);
    }

    #[test]
    fn the_data_block_is_wrapped_in_the_magic_byte_and_the_checksum() {
        let layer = layer_of(0);
        let size_at = DEFINITION_BYTES as usize;
        let size = u32::from_be_bytes([
            layer[size_at],
            layer[size_at + 1],
            layer[size_at + 2],
            layer[size_at + 3],
        ]) as usize;

        assert_eq!(layer[size_at + 4], LAYER_MAGIC);
        assert_eq!(layer.len(), size_at + 4 + size + 2);
        assert_eq!(&layer[layer.len() - 2..], &DELIMITER);
    }

    #[test]
    fn a_layer_states_both_stages_and_the_bottom_light_off() {
        let mut job = sample_job(10);
        job.material.waits.mode = printer_profiles::WaitMode::LightOff;
        job.material.bottom_light_off_delay_s = Some(5.0);
        job.material.second_stage.bottom_lift_distance_mm = 3.0;
        job.material.second_stage.bottom_lift_speed_mm_min = 180.0;
        let encoded = EncodedLayer::encode(&LayerRuns::builder(8, 4).finish());
        let mut buffer = Cursor::new(Vec::new());
        write(&mut Fields::new(&mut buffer), &job, 0, &encoded).expect("in-memory write");
        let layer = buffer.into_inner();

        assert!(
            (float_at(&layer, 14) - 5.0).abs() < 1e-6,
            "the bottom light-off"
        );
        assert_eq!((float_at(&layer, 38), float_at(&layer, 42)), (3.0, 180.0));
    }
}
