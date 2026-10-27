use std::io::Cursor;

use core_format::{FormatError, LayerSink, OpenFile, SlicedFileReader, SlicedFileWriter};
use core_raster::LayerRuns;

use super::reader::crypt;
use super::{PhzReader, PhzWriter};
use crate::fdg::{FdgSink, HEADER_BYTES};
use crate::fixtures::{sample_job, two_stage_job};

/// An 8 x 4 layer with a lit block from column 2 to 5 in rows 1 and 2.
fn block() -> LayerRuns {
    let mut builder = LayerRuns::builder(8, 4);
    builder.push(10, 0);
    builder.push(4, 255);
    builder.push(4, 0);
    builder.push(4, 255);
    builder.push(10, 0);
    builder.finish()
}

fn written(layers: u32) -> Vec<u8> {
    let mut job = sample_job(layers);
    job.material.light_pwm = 200;
    job.material.bottom_light_pwm = 100;
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = PhzWriter.begin(&job, &mut file).expect("the job is sound");
        for _ in 0..layers {
            sink.push(FdgSink::encode(&block())).expect("a layer");
        }
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    file.into_inner()
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[test]
fn the_header_names_its_blocks_where_the_vendor_writer_does() {
    let bytes = written(3);
    assert_eq!((word(&bytes, 0), word(&bytes, 4)), (0x9FDA_83AE, 4));
    assert_eq!(
        word(&bytes, 0x20),
        HEADER_BYTES as u32,
        "the large preview follows the header"
    );
    assert_eq!(word(&bytes, 0x28), 3);
    assert_eq!(
        word(&bytes, 0x3C),
        0x0064_00C8,
        "the light PWM, then the bottom one"
    );
    assert_eq!(
        word(&bytes, 0x64),
        word(&bytes, 0x14),
        "the bottom layers again"
    );
    assert_eq!(word(&bytes, 0xB0), 0x14);
    assert_eq!((word(&bytes, 0xCC), word(&bytes, 0xD0)), (0xD8, 0x50));

    let (name_at, name_bytes) = (word(&bytes, 0x90) as usize, word(&bytes, 0x94) as usize);
    assert_eq!(&bytes[name_at..name_at + name_bytes], b"Test");
}

#[test]
fn a_written_file_reads_back() {
    let mut open = PhzReader
        .open(Cursor::new(written(3)))
        .expect("a .phz we wrote");
    let facts = open.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (8, 4));
    assert_eq!(facts.machine.as_deref(), Some("Test"));
    assert_eq!(facts.volume_mm3, Some(1500.0));
    assert_eq!(facts.stated.light_pwm, Some(200));
    assert_eq!(facts.stated.bottom_light_pwm, Some(100));

    let lit: u32 = open
        .layer(2)
        .expect("the last layer")
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 8);
}

#[test]
fn the_cipher_undoes_itself_and_zero_leaves_the_bytes_alone() {
    let clear: Vec<u8> = (0..23).collect();
    let mut covered = clear.clone();
    crypt(0x1234_5678, 7, &mut covered);
    assert_ne!(covered, clear);
    crypt(0x1234_5678, 7, &mut covered);
    assert_eq!(covered, clear);

    let mut untouched = clear.clone();
    crypt(0, 7, &mut untouched);
    assert_eq!(untouched, clear);
}

#[test]
fn the_cipher_steps_by_the_key_and_the_layer_as_the_vendor_writer_does() {
    // Key 1 and layer 0: the step is 0x34A32231, the first word that times
    // 0x3FAD2212 * 0x1E1530CD, and each word after it one step more.
    let mut data = [0u8; 8];
    crypt(1, 0, &mut data);
    let step = 0x34A3_2231u32;
    let first = step.wrapping_mul(0x3FAD_2212u32.wrapping_mul(0x1E15_30CD));
    assert_eq!(data[..4], first.to_le_bytes());
    assert_eq!(data[4..], first.wrapping_add(step).to_le_bytes());
}

#[test]
fn a_written_file_covers_its_layers_with_a_key_and_reads_back() {
    let bytes = written(2);
    assert_ne!(word(&bytes, 0x58), 0, "the vendor never writes a zero key");
    let mut open = PhzReader.open(Cursor::new(bytes)).expect("the header");
    let lit: u32 = open
        .layer(1)
        .expect("a covered layer")
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 8);
}

#[test]
fn a_file_of_another_container_is_refused_by_its_magic() {
    let err = PhzReader
        .open(Cursor::new(0xBD3C_7AC8u32.to_le_bytes().to_vec()))
        .err()
        .expect("an .fdg magic");
    assert!(matches!(
        err,
        FormatError::NotThisFormat {
            format: "phz",
            found: 0xBD3C_7AC8
        }
    ));
}

#[test]
fn the_header_states_one_pass_then_the_anti_alias_level() {
    let bytes = written(1);
    assert_eq!((word(&bytes, 0x38), word(&bytes, 0xB8)), (1, 16));
}

#[test]
fn a_two_stage_peel_states_both_climbs_summed_then_the_second_stage() {
    let job = two_stage_job(1);
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = PhzWriter.begin(&job, &mut file).expect("the job is sound");
        sink.push(FdgSink::encode(&block())).expect("a layer");
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    let bytes = file.into_inner();
    let f32_at = |at: usize| f32::from_bits(word(&bytes, at));
    assert_eq!(
        (f32_at(0x6C), f32_at(0x74)),
        (8.0, 6.0),
        "bottom lift 5 + 3 mm, lift 2 + 4 mm"
    );
    let second: Vec<f32> = (0x98..0xB0).step_by(4).map(f32_at).collect();
    assert_eq!(second, [3.0, 180.0, 4.0, 900.0, 4.0, 360.0]);
}
