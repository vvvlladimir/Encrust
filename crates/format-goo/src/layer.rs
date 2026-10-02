use std::io;

use crate::header::DELIMITER;
use crate::rle::EncodedLayer;
use core_format::Fields;
use core_format::PrintJob;

/// Marks the start of a layer's run-length data; the checksum does not cover it.
const LAYER_MAGIC: u8 = 0x55;

pub(crate) fn write(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    index: u32,
    encoded: &EncodedLayer,
) -> io::Result<()> {
    write_definition(fields, job, index)?;
    fields.bytes(&DELIMITER)?;

    fields.u32_be(encoded.data_size())?;
    fields.u8(LAYER_MAGIC)?;
    fields.bytes(encoded.data())?;
    fields.u8(encoded.checksum())?;
    fields.bytes(&DELIMITER)
}

fn write_definition(fields: &mut Fields<'_>, job: &PrintJob, index: u32) -> io::Result<()> {
    let material = &job.material;
    let bottom = material.is_bottom_layer(index);

    // Pause position, then the layer position. A layer that pauses stops where it is, as a
    // vendor file carries it; the machine's Z travel here moves the plate off the print.
    fields.u16_be(0)?;
    fields.f32_be(job.layer_z_mm(index))?;
    fields.f32_be(job.layer_z_mm(index))?;
    fields.f32_be(job.exposure_of_layer_s(index))?;
    fields.f32_be(material.light_off_s())?;
    for rest_s in material.waits.rests_s() {
        fields.f32_be(rest_s)?;
    }

    let (lift_distance, lift_speed) = if bottom {
        (
            material.bottom_lift_distance_mm,
            material.bottom_lift_speed_mm_min,
        )
    } else {
        (material.lift_distance_mm, material.lift_speed_mm_min)
    };
    let (retract_distance, retract_speed) = if bottom {
        (
            material.bottom_lift_distance_mm,
            material.bottom_retract_speed_mm_min,
        )
    } else {
        (material.retract_distance_mm, material.retract_speed_mm_min)
    };

    fields.f32_be(lift_distance)?;
    fields.f32_be(lift_speed)?;
    fields.f32_be(0.0)?;
    fields.f32_be(0.0)?;
    fields.f32_be(retract_distance)?;
    fields.f32_be(retract_speed)?;
    fields.f32_be(0.0)?;
    fields.f32_be(0.0)?;

    fields.u16_be(u16::from(if bottom {
        material.bottom_light_pwm
    } else {
        material.light_pwm
    }))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use core_raster::LayerRuns;

    use super::*;
    use crate::fixtures::sample_job;

    /// Bytes before the data size field: the pause flag, fifteen floats, the light PWM
    /// and the delimiter that closes the definition.
    const DEFINITION_BYTES: usize = 2 + 15 * 4 + 2 + 2;

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
        let size_at = DEFINITION_BYTES;
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
}
