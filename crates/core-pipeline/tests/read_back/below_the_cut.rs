//! Containers that cannot carry the stack's grey: the one-pixel border of grey 120 falls
//! below their cut, and each keeps the block inside it and what it holds of the resin.

use super::*;

/// The vector container, which carries no grey at all: a path is filled or it is not, so the
/// border of grey 120 is below the cut and the block inside it comes back whole.
#[test]
fn the_vector_container_keeps_what_it_can_carry() {
    let bytes = write_with(&format_svgx::SvgxWriter);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes as well"
    );

    let mut source = Cursor::new(bytes);
    let mut opened = open(Path::new("plate.svgx"), &mut source).expect("an .svgx we wrote opens");

    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert!(
        (facts.exposure_s - 2.5).abs() < 1e-3,
        "{}",
        facts.exposure_s
    );

    let runs = opened.layer(0).expect("the first layer");
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    assert_eq!(covered, u64::from(WIDTH_PX) * u64::from(HEIGHT_PX));

    let greys: std::collections::BTreeSet<u8> = runs
        .iter()
        .map(|run| run.value)
        .filter(|v| *v > 0)
        .collect();
    assert_eq!(
        greys.len(),
        1,
        "one grey, which is full exposure: {greys:?}"
    );

    // The 48 by 24 block less the one-pixel border the container cannot hold.
    assert_eq!(lit_px(&runs), 46 * 22, "the border falls below the cut");
}

/// The one-bit container: the border of grey 120 is below its cut, and the resin comes
/// back as `.ctb` states it.
#[test]
fn the_one_bit_container_keeps_the_lit_area_and_the_resin() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_chitu::FdgWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened = open(Path::new("plate.fdg"), &mut source).expect("an .fdg we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert_eq!(facts.stated, stated_by("ctb"));

    let runs = opened.layer(0).expect("the first layer");
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    assert_eq!(covered, u64::from(WIDTH_PX) * u64::from(HEIGHT_PX));
    assert_eq!(lit_px(&runs), 46 * 22, "the border falls below the cut");
}

/// The Photon S's container: one bit a pixel, and of the resin only its peel and wait.
#[test]
fn the_photon_s_container_keeps_the_lit_area_and_the_peel() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_anycubic::PhotonsWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened =
        open(Path::new("plate.photons"), &mut source).expect("a .photons we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert_eq!(facts.stated, stated_by("cws"), "the same peel as the .cws");

    let runs = opened.layer(0).expect("the first layer");
    assert_eq!(lit_px(&runs), 46 * 22, "the border falls below the cut");
}

/// The Sonic Mini's container: the `.fdg` layout under another header, the resin as `.ctb`
/// states it.
#[test]
fn the_sonic_mini_container_keeps_the_lit_area_and_the_resin() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_chitu::PhzWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened = open(Path::new("plate.phz"), &mut source).expect("a .phz we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    assert_eq!(facts.stated, stated_by("ctb"));

    let runs = opened.layer(0).expect("the first layer");
    assert_eq!(lit_px(&runs), 46 * 22, "the border falls below the cut");
}

/// The D158's container: polygons traced out of the mask, so the border of grey 120 is below
/// the cut and the block inside it comes back whole; the light is stated as it was told.
#[test]
fn the_polygon_container_of_the_creality_family_keeps_the_lit_area() {
    let job = PrintJob {
        material: told(),
        ..job()
    };
    let bytes = write_job(&format_creality::CxlineWriter, &job);
    let mut unnamed = Cursor::new(bytes.clone());
    assert!(
        open(Path::new("plate.bin"), &mut unnamed).is_ok(),
        "it names itself in its bytes"
    );

    let mut source = Cursor::new(bytes);
    let mut opened =
        open(Path::new("plate.cxline"), &mut source).expect("a .cxline we wrote opens");
    let facts = opened.facts();
    assert_eq!(facts.layer_count(), LAYERS);
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.grey_steps, 2);
    let told = stated_by("goo");
    assert_eq!(
        (facts.stated.light_pwm, facts.stated.bottom_light_pwm),
        (told.light_pwm, told.bottom_light_pwm)
    );

    let runs = opened.layer(0).expect("the first layer");
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    assert_eq!(covered, u64::from(WIDTH_PX) * u64::from(HEIGHT_PX));
    assert_eq!(lit_px(&runs), 46 * 22, "the border falls below the cut");
}
