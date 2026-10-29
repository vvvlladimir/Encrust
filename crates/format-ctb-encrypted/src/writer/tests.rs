//! Every expected number is what a known GK3 Ultra file with the same resin states,
//! deciphered.

use core_format::Crc32;
use core_raster::Shading;

use super::*;
use crate::fixtures::{test_resin_job, written};

/// Bytes behind each layer's data: its lit area and a zero word.
const LAYER_TAIL_BYTES: usize = 8;

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}

fn deciphered(file: &[u8]) -> Settings {
    let start = HEADER_BYTES as usize;
    let mut settings = Settings(
        file[start..start + SETTINGS_BYTES]
            .try_into()
            .expect("the block is whole"),
    );
    cipher::decrypt(&mut settings.0, "settings").expect("our own settings decipher");
    settings
}

/// Layer `index`'s 88-byte definition, found through the pointer table.
fn definition(file: &[u8], settings: &Settings, index: u32) -> Vec<u8> {
    let pointer =
        settings.u32(at::LAYER_POINTERS) as usize + POINTER_BYTES as usize * index as usize;
    let at = word(file, pointer) as usize;
    file[at..at + LAYER_DEF_BYTES as usize].to_vec()
}

fn floats(bytes: &[u8], at: usize, count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| float(bytes, at + 4 * index))
        .collect()
}

#[test]
fn the_settings_state_both_stages_and_the_whole_climb() {
    let settings = deciphered(&written(&test_resin_job(4)));
    assert_eq!(
        settings.f32(at::BOTTOM_LIFT_MM),
        8.0,
        "5 + 3 mm, both stages"
    );
    assert_eq!(settings.f32(at::LIFT_MM), 6.0, "2 + 4 mm, both stages");
    assert_eq!(
        floats(&settings.0, at::SECOND_STAGE, 6),
        [3.0, 180.0, 4.0, 900.0, 4.0, 360.0]
    );
    assert_eq!(settings.f32(at::BOTTOM_RETRACT_SPEED_2), 180.0);
    assert_eq!(settings.f32(at::BOTTOM_RETRACT_MM_2), 3.0);
}

#[test]
fn the_bottom_layers_take_their_own_light_off_delay() {
    let file = written(&test_resin_job(4));
    let settings = deciphered(&file);
    assert_eq!(settings.f32(at::LIGHT_OFF_S), 0.5);
    assert_eq!(settings.f32(at::BOTTOM_LIGHT_OFF_S), 5.0);
    assert_eq!(float(&definition(&file, &settings, 0), 0x0C), 5.0);
    assert_eq!(float(&definition(&file, &settings, 2), 0x0C), 0.5);
}

#[test]
fn each_definition_states_its_layers_peel() {
    let file = written(&test_resin_job(4));
    let settings = deciphered(&file);
    assert_eq!(
        floats(&definition(&file, &settings, 0), 0x28, 7),
        [8.0, 120.0, 3.0, 180.0, 240.0, 3.0, 180.0],
        "a bottom layer"
    );
    assert_eq!(
        floats(&definition(&file, &settings, 2), 0x28, 7),
        [6.0, 360.0, 4.0, 900.0, 900.0, 4.0, 360.0],
        "a normal layer"
    );
}

#[test]
fn the_settings_close_on_the_stacks_totals() {
    let job = test_resin_job(4);
    let file = written(&job);
    let settings = deciphered(&file);
    // Two bottom layers of 30 s and 5 s dark, two of 2.5 s and 0.5 s.
    assert_eq!(settings.f32(at::LAYER_TIME_S), 76.0);
    let data_bytes = definition(&file, &settings, 0)[0x18..0x1C].to_vec();
    assert_eq!(
        settings.u32(at::LARGEST_LAYER_BYTES),
        u32::from_le_bytes(data_bytes.try_into().expect("a word"))
    );
    assert_eq!(settings.u32(at::LAYER_COUNT_AGAIN), 4);
    assert_eq!(settings.u32(0xF0), 0);
    assert_eq!(settings.u32(at::ONE_AGAIN), 1);
    assert_eq!(settings.f32(0x20), 0.0, "the format states no total height");
    let cost = job.cost().expect("the resin has a price");
    assert!((settings.f32(at::COST) - cost).abs() < 1e-6, "{cost}");
}

#[test]
fn a_stack_the_settings_describe_leaves_the_per_layer_flag_clear() {
    let mut job = test_resin_job(8);
    job.material.transition_layers = 3;
    let settings = deciphered(&written(&job));
    assert_eq!(
        settings.u32(at::FLAGS),
        7,
        "binary grey, no per-layer settings"
    );
    assert_eq!(settings.u32(at::ANTI_ALIAS_LEVEL), 4, "the dormant level");
}

#[test]
fn a_grey_stack_states_the_flag_a_binary_stack_states() {
    let mut job = test_resin_job(2);
    job.raster.shading = Shading::Coverage;
    assert_eq!(deciphered(&written(&job)).u32(at::FLAGS), 7);
}

#[test]
fn every_layer_is_followed_by_the_area_it_lights() {
    let file = written(&test_resin_job(3));
    let settings = deciphered(&file);
    for index in 0..2 {
        let def = definition(&file, &settings, index);
        let end = word(&def, 0x10) as usize + word(&def, 0x18) as usize;
        // Sixteen lit pixels of 0.1 x 0.1 mm.
        assert!((float(&file, end) - 0.16).abs() < 1e-6, "layer {index}");
        assert_eq!(word(&file, end + 4), 0);
        let next = word(
            &file,
            settings.u32(at::LAYER_POINTERS) as usize + 16 * (index as usize + 1),
        );
        assert_eq!(next as usize, end + LAYER_TAIL_BYTES, "layer {index}");
    }
}

#[test]
fn the_file_ends_with_the_crc_of_everything_before_it() {
    let file = written(&test_resin_job(3));
    let (body, last) = file.split_at(file.len() - 4);
    let mut crc = Crc32::default();
    crc.update(body);
    assert_eq!(word(last, 0), crc.zlib());
}
