use std::io::Cursor;

use super::*;
use crate::fixtures::sample_job;

fn header_of(job: &PrintJob) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let mut fields = Fields::new(&mut buffer);
    write(&mut fields, job).expect("in-memory write");
    buffer.into_inner()
}

/// Where the exposure delay mode sits, then the light-off time, then the six waits.
const DELAY_MODE_OFFSET: usize = 195_340;

fn f32_at(header: &[u8], offset: usize) -> f32 {
    f32::from_be_bytes([
        header[offset],
        header[offset + 1],
        header[offset + 2],
        header[offset + 3],
    ])
}

#[test]
fn a_resting_resin_selects_the_static_waits_and_fills_them() {
    let mut job = sample_job(10);
    job.material.waits = printer_profiles::Waits {
        mode: printer_profiles::WaitMode::Rest,
        before_lift_s: 1.5,
        after_lift_s: 0.5,
        after_retract_s: 2.0,
    };
    let header = header_of(&job);
    assert_eq!(
        header[DELAY_MODE_OFFSET], 1,
        "static waits, not turn-off time"
    );
    assert!(
        f32_at(&header, DELAY_MODE_OFFSET + 1).abs() < f32::EPSILON,
        "no light-off delay"
    );
    let waits: Vec<f32> = (0..6)
        .map(|index| f32_at(&header, DELAY_MODE_OFFSET + 5 + 4 * index))
        .collect();
    assert_eq!(
        waits,
        [1.5, 0.5, 2.0, 1.5, 0.5, 2.0],
        "bottom layers, then the rest"
    );
}

#[test]
fn a_resin_holding_the_light_off_writes_no_waits() {
    let job = sample_job(10);
    let header = header_of(&job);
    assert_eq!(header[DELAY_MODE_OFFSET], 0);
    assert!((f32_at(&header, DELAY_MODE_OFFSET + 1) - job.material.light_off_delay_s).abs() < 1e-6);
}

/// Both machine-name fields, the first straight after the three 32-byte strings.
const MACHINE_NAME_OFFSET: usize = 4 + 8 + 32 + 24 + 24;

/// Total price, then the eight-byte unit it is quoted in.
const PRICE_OFFSET: usize = 195_458;

fn text_at(header: &[u8], offset: usize, width: usize) -> String {
    let field = &header[offset..offset + width];
    let end = field.iter().position(|&b| b == 0).unwrap_or(width);
    String::from_utf8(field[..end].to_vec()).expect("a header string is utf-8")
}

#[test]
fn both_machine_fields_carry_the_name_the_firmware_matches() {
    let mut job = sample_job(10);
    job.printer.name = "Saturn 4 Ultra".to_owned();
    job.printer.machine_name = Some("ELEGOO Saturn 4 Ultra".to_owned());
    let header = header_of(&job);
    assert_eq!(
        text_at(&header, MACHINE_NAME_OFFSET, 32),
        "ELEGOO Saturn 4 Ultra"
    );
    assert_eq!(
        text_at(&header, MACHINE_NAME_OFFSET + 32, 32),
        "ELEGOO Saturn 4 Ultra"
    );
}

#[test]
fn the_totals_carry_what_the_resin_costs_and_what_it_is_quoted_in() {
    let mut job = sample_job(10);
    job.volume_mm3 = 1_000_000.0;
    job.material.details.price = 20.0;
    job.material.details.currency = "€".to_owned();
    job.material.details.price_per = printer_profiles::PriceUnit::Litre;
    let header = header_of(&job);
    assert!(
        (f32_at(&header, PRICE_OFFSET) - 20.0).abs() < 1e-3,
        "a litre of resin at 20 a litre"
    );
    assert_eq!(text_at(&header, PRICE_OFFSET + 4, 8), "€/L");
}

#[test]
fn the_header_is_exactly_the_size_the_format_fixes() {
    let header = header_of(&sample_job(10));
    assert_eq!(
        header.len(),
        HEADER_SIZE as usize,
        "the format fixes the header at 0x2FB95 bytes and layer content starts there"
    );
}

#[test]
fn the_header_opens_with_the_version_and_the_magic_tag() {
    let header = header_of(&sample_job(10));
    assert_eq!(&header[..4], b"V3.0");
    assert_eq!(&header[4..12], &MAGIC_TAG);
}

#[test]
fn the_layer_content_offset_is_the_header_size() {
    let header = header_of(&sample_job(10));
    let offset = HEADER_SIZE as usize - 7;
    assert_eq!(
        u32::from_be_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3]
        ]),
        HEADER_SIZE
    );
}

/// Anti-aliasing level sits after the eight fixed-width strings, the grey level next.
const AA_OFFSET: usize = 4 + 8 + 32 + 24 + 24 + 32 + 32 + 32;

fn u16_at(header: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([header[offset], header[offset + 1]])
}

#[test]
fn binary_shading_is_reported_as_a_single_anti_aliasing_level() {
    let mut job = sample_job(10);
    job.raster.shading = Shading::Binary;
    let header = header_of(&job);
    assert_eq!(u16_at(&header, AA_OFFSET), 1);
    assert_eq!(u16_at(&header, AA_OFFSET + 2), 1, "and a single grey level");
}

#[test]
fn the_grey_level_is_the_count_the_masks_were_rounded_to() {
    let mut job = sample_job(10);
    job.raster.grey.levels = std::num::NonZeroU8::new(4);
    let header = header_of(&job);
    assert_eq!(u16_at(&header, AA_OFFSET), 8, "the masks are still 8-bit");
    assert_eq!(u16_at(&header, AA_OFFSET + 2), 4);
}

#[test]
fn unrounded_grey_is_reported_as_no_level_count_at_all() {
    let header = header_of(&sample_job(10));
    assert_eq!(u16_at(&header, AA_OFFSET + 2), 0);
}

#[test]
fn the_blur_level_is_the_radius_the_edges_were_faded_over() {
    let mut job = sample_job(10);
    assert_eq!(
        u16_at(&header_of(&job), AA_OFFSET + 4),
        0,
        "sharp by default"
    );
    job.raster.blur_px = 2;
    assert_eq!(u16_at(&header_of(&job), AA_OFFSET + 4), 2);
    job.raster.shading = Shading::Binary;
    assert_eq!(
        u16_at(&header_of(&job), AA_OFFSET + 4),
        0,
        "a binary mask is never blurred"
    );
}
