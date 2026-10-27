use super::*;
use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
use printer_profiles::PrinterTuning;
use std::path::Path;
use std::sync::Arc;

use crate::scene::ImportSummary;

const PROFILE: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 64
height_px = 32
width_mm = 12.8
height_mm = 6.4

[build_volume]
x = 12.8
y = 6.4
z = 10.0
"#;

fn tetrahedron() -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    )
}

fn scene_with_a_model() -> Scene {
    let mut scene = Scene::default();
    let mesh = tetrahedron();
    scene.insert(crate::scene::Imported::new(
        "tetra".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));
    scene
}

fn with_printer() -> Slicing {
    let mut slicing = Slicing {
        printer: Some(
            PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
                .expect("the inline profile is valid"),
        ),
        ..Slicing::default()
    };
    // Nothing ships an exposure, so a cut needs a resin in hand as much as a panel.
    slicing.set_material(MaterialProfile::default());
    slicing
}

/// A resin measured at 0.05 mm for 2.5 s, with one band of 4 s over it.
fn measured_resin() -> Slicing {
    let mut slicing = Slicing::default();
    slicing.set_material(MaterialProfile {
        layer_height_mm: 0.05,
        exposure_s: 2.5,
        penetration_depth_mm: None,
        ..MaterialProfile::default()
    });
    slicing.exposure = vec![ExposureRange::new(5.0, 10.0, 4.0)];
    slicing
}

#[test]
fn a_new_layer_height_carries_the_exposures_along_and_remembers_them() {
    let mut slicing = measured_resin();
    slicing.set_layer_height(0.1);

    // Jacobs at the assumed 0.1 mm depth: half a depth thicker is exp(0.5) the dose.
    assert!((slicing.material.exposure_s - 4.1218).abs() < 1e-3);
    assert!((slicing.exposure[0].exposure_s - 6.5949).abs() < 1e-3);
    let rescaled = slicing.rescaled().expect("the change is kept to be shown");
    assert!((rescaled.from_mm - 0.05).abs() < 1e-6);
    assert!((rescaled.exposure_s - 2.5).abs() < 1e-6);
    assert!(
        (slicing.material.bottom_exposure_s - MaterialProfile::default().bottom_exposure_s).abs()
            < 1e-6,
        "the bottom block is exposed to stick, not to cure through"
    );
}

#[test]
fn going_back_puts_the_exposures_back_and_keeps_the_new_height() {
    let mut slicing = measured_resin();
    slicing.set_layer_height(0.1);
    slicing.revert_exposure();

    assert!((slicing.layer_height_mm() - 0.1).abs() < 1e-6);
    assert!((slicing.material.exposure_s - 2.5).abs() < 1e-6);
    assert!((slicing.exposure[0].exposure_s - 4.0).abs() < 1e-6);
    assert_eq!(slicing.rescaled(), None, "nothing is left to point at");
}

#[test]
fn two_changes_go_back_to_what_was_set_before_the_first() {
    let mut slicing = measured_resin();
    slicing.set_layer_height(0.1);
    slicing.set_layer_height(0.03);
    let rescaled = slicing.rescaled().expect("still carried");
    assert!((rescaled.exposure_s - 2.5).abs() < 1e-6);
    assert!((slicing.material.exposure_s - 2.0468).abs() < 1e-3);

    slicing.set_layer_height(0.05);
    assert_eq!(slicing.rescaled(), None, "back where it was measured");
    assert_eq!(
        slicing.material.exposure_s, 2.5,
        "exactly, not carried round"
    );
}

#[test]
fn an_exposure_set_by_hand_stops_being_pointed_at() {
    let mut slicing = measured_resin();
    slicing.set_layer_height(0.1);
    slicing.material.exposure_s = 4.2;
    slicing.exposure_edited();
    slicing.revert_exposure();
    assert_eq!(slicing.material.exposure_s, 4.2);
}

#[test]
fn carrying_the_resin_along_writes_the_same_exposures() {
    let before = measured_resin();
    let mut after = measured_resin();
    after.set_layer_height(0.03);

    let printer = PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
        .expect("the inline profile is valid");
    let job = |slicing: &Slicing| core_format::PrintJob {
        printer: printer.clone(),
        material: slicing.material.clone(),
        raster: raster_settings(&printer, PanelOverrides::default()),
        plan: core_slicer::LayerPlan::of_count(0.03, 400),
        volume_mm3: 0.0,
        exposure: ExposurePlan::new(slicing.exposure.clone()),
        thumbnail: None,
        created_unix_s: 0,
    };
    let (before, after) = (job(&before), job(&after));
    for index in [0, 10, 100, 250, 399] {
        let (was, is) = (
            before.exposure_of_layer_s(index),
            after.exposure_of_layer_s(index),
        );
        assert!(
            (was - is).abs() < 1e-4,
            "layer {index}: {was} s, now {is} s"
        );
    }
}

#[test]
fn a_project_keeps_the_bands_at_the_height_the_resin_was_measured_at() {
    let mut slicing = measured_resin();
    slicing.set_layer_height(0.1);
    let kept = slicing.bands_as_measured();
    assert!(
        (kept[0].exposure_s - 4.0).abs() < 1e-5,
        "got {}",
        kept[0].exposure_s
    );
}

#[test]
fn the_layer_height_follows_the_material() {
    let mut slicing = Slicing::default();
    slicing.set_material(MaterialProfile {
        layer_height_mm: 0.03,
        ..MaterialProfile::default()
    });
    assert!((slicing.layer_height_mm() - 0.03).abs() < 1e-6);
}

#[test]
fn slicing_without_a_printer_profile_is_blocked() {
    let mut slicing = Slicing::default();
    let scene = scene_with_a_model();
    assert!(slicing.blocker(&scene).is_some());
    assert!(slicing.start(&scene, 0, PathBuf::from("out.goo")).is_err());
    assert!(slicing.job.is_none(), "a rejected start runs nothing");
}

#[test]
fn slicing_while_the_supports_are_growing_is_blocked() {
    let mut slicing = with_printer();
    let mut scene = scene_with_a_model();
    let object = &mut scene.objects_mut()[0];
    object
        .supports
        .add(Vec3::new(0.2, 0.2, 0.0), object.transform, 0);

    assert_eq!(
        slicing.blocker(&scene),
        Some(tr!("slicing-supports-growing"))
    );
    assert!(slicing.start(&scene, 0, PathBuf::from("out.goo")).is_err());
    assert!(slicing.job.is_none(), "a rejected start runs nothing");
}

#[test]
fn slicing_an_empty_plate_is_blocked() {
    let mut slicing = with_printer();
    let scene = Scene::default();
    assert!(slicing.blocker(&scene).is_some());
    assert!(slicing.start(&scene, 0, PathBuf::from("out.goo")).is_err());
}

#[test]
fn a_name_no_format_claims_is_refused_before_anything_is_written() {
    let mut slicing = with_printer();
    let output =
        std::env::temp_dir().join(format!("encrust-{}-no-format.sliced", std::process::id()));
    assert!(
        slicing
            .start(&scene_with_a_model(), 0, output.clone())
            .is_err()
    );
    assert!(slicing.job.is_none(), "a rejected start runs nothing");
    assert!(!output.exists(), "nor does it leave a file or a PNG stack");
}

#[test]
fn a_hidden_model_is_not_something_to_slice() {
    let slicing = with_printer();
    let mut scene = scene_with_a_model();
    scene.objects_mut()[0].visible = false;
    assert!(slicing.blocker(&scene).is_some());
}

#[test]
fn a_loaded_printer_and_a_visible_model_are_enough() {
    let slicing = with_printer();
    assert_eq!(slicing.blocker(&scene_with_a_model()), None);
}

#[test]
fn exposure_bands_are_blocked_on_a_printer_that_reads_the_header_alone() {
    let mut slicing = with_printer();
    slicing.exposure = vec![ExposureRange::new(0.0, 5.0, 4.0)];
    assert_eq!(slicing.blocker(&scene_with_a_model()), None);

    if let Some(printer) = slicing.printer.as_mut() {
        printer.firmware.per_layer_settings = false;
    }
    assert!(slicing.blocker(&scene_with_a_model()).is_some());
}

#[test]
fn adaptive_layers_are_blocked_on_a_printer_that_steps_by_the_header() {
    let mut slicing = with_printer();
    slicing.adaptive = Some(AdaptiveSettings::default());
    assert!(slicing.blocker(&scene_with_a_model()).is_some());
    assert_eq!(slicing.cutting().adaptive, slicing.adaptive);

    if let Some(printer) = slicing.printer.as_mut() {
        printer.firmware.variable_layer_height = true;
    }
    assert_eq!(slicing.blocker(&scene_with_a_model()), None);
}

#[test]
fn anti_aliasing_decides_the_shading() {
    let mut slicing = Slicing::default();
    assert_eq!(slicing.shading(), Shading::Coverage);
    slicing.anti_alias = false;
    assert_eq!(slicing.shading(), Shading::Binary);
}

#[test]
fn a_finished_job_names_the_file_and_a_failed_one_shows_red() {
    let resin = MaterialProfile::default();
    let written = report(
        Outcome::Written {
            path: PathBuf::from("cube.goo"),
            layers: 40,
            clipped_layers: 0,
            volume_mm3: 2000.0,
        },
        &resin,
    );
    assert!(!written.is_error());
    assert!(written.text().contains("40 layers"));
    assert!(written.text().contains("2.0 ml"));
    assert!(written.text().contains("2.2 g"), "at the resin's 1.1 g/cm3");

    let clipped = report(
        Outcome::Written {
            path: PathBuf::from("cube.goo"),
            layers: 40,
            clipped_layers: 3,
            volume_mm3: 2000.0,
        },
        &resin,
    );
    assert!(clipped.text().contains("cut off"));

    assert!(report(Outcome::Failed("cannot create".to_owned()), &resin).is_error());
    assert!(!report(Outcome::Cancelled, &resin).is_error());
}

#[test]
fn a_priced_resin_puts_what_the_print_cost_in_the_report() {
    let mut resin = MaterialProfile::default();
    resin.details.price = 20.0;
    let written = report(
        Outcome::Written {
            path: PathBuf::from("cube.goo"),
            layers: 40,
            clipped_layers: 0,
            volume_mm3: 50_000.0,
        },
        &resin,
    );
    assert!(written.text().contains("1.10 €"), "55 g at 20 a kilo");
}

#[test]
fn a_run_over_the_project_queues_only_the_plates_with_something_on_them() {
    let mut scene = Scene::default();
    scene.add_plate();
    let mut slicing = Slicing::default();
    let mut status = Status::default();

    // No printer, so nothing actually starts; what is under test is what was queued.
    assert_eq!(
        slicing.start_all(&scene, &PathBuf::from("/tmp/out.goo"), &mut status),
        0,
        "an empty project has no plate worth cutting"
    );
}

#[test]
fn polling_without_a_job_reports_nothing_running() {
    let mut slicing = Slicing::default();
    let mut status = Status::default();
    assert!(!slicing.poll(&Scene::default(), &mut status));
    assert_eq!(status, Status::Idle);
}

#[test]
fn a_stack_cut_to_send_is_still_for_sending_when_the_window_asked_every_frame() {
    let scene = scene_with_a_model();
    let mut slicing = with_printer();
    let mut status = Status::default();
    let path = std::env::temp_dir().join("encrust-slicing-to-send.goo");
    slicing.send_when_written();
    slicing
        .start(&scene, 0, path.clone())
        .expect("a loaded printer and a model");
    let written = loop {
        // The window asks for a written file on every frame, the job's first included.
        if let Some(written) = slicing.take_written() {
            break written;
        }
        slicing.poll(&scene, &mut status);
        assert!(!status.is_error(), "{status:?}");
    };
    std::fs::remove_file(&path).ok();
    assert!(written.to_send, "the Send button asked for this stack");
    assert!(
        !slicing.to_send,
        "the next stack goes to a file unless asked again"
    );
}

const MARS: &str = "elegoo-mars-3-pro";
const SATURN: &str = "elegoo-saturn-4-ultra";

/// A catalogue of its own holding one resin the user measured on both Elegoo
/// machines, which is what a window someone has set a machine up in looks like.
/// Nothing is shipped to install; see `docs/decisions/0196`.
fn with_a_measured_resin(name: &str) -> Catalogue {
    let dir = std::env::temp_dir().join(format!("encrust-slicing-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    catalogue
        .save_resin("grey", &measured_on_both())
        .expect("the directory is writable");
    catalogue
}

/// 2.6 s of its own, 3.2 s on the Mars 3 Pro's LED matrix and 2.3 s on the Saturn's
/// COB engine, as an exposure test on the two machines gives.
fn measured_on_both() -> MaterialProfile {
    let mut resin = MaterialProfile {
        name: "Grey".to_owned(),
        exposure_s: 2.6,
        ..MaterialProfile::default()
    };
    for (printer, exposure_s) in [(MARS, 3.2), (SATURN, 2.3)] {
        resin.printers.insert(
            printer.to_owned(),
            PrinterTuning {
                exposure_s: Some(exposure_s),
                ..PrinterTuning::default()
            },
        );
    }
    resin.last_printer = Some(MARS.to_owned());
    resin
}

#[test]
fn picking_a_printer_takes_the_format_its_firmware_reads_and_a_resin_measured_on_it() {
    let mut slicing = Slicing {
        catalogue: with_a_measured_resin("picking"),
        ..Slicing::default()
    };
    let chitu = slicing.catalogue.printer(MARS).expect("shipped");
    let (profile, id) = (chitu.profile.clone(), chitu.id.clone());

    slicing.set_printer(profile, Some(id));

    assert_eq!(slicing.format, SlicedFormat::CtbEncrypted);
    assert!(slicing.resin_id.is_some(), "a printer brings a resin");
    assert!(
        slicing.resin_is_tuned(),
        "the resin it brings is one measured on it"
    );
}

#[test]
fn a_resin_is_retuned_when_the_printer_under_it_changes() {
    let mut slicing = Slicing {
        catalogue: with_a_measured_resin("retune"),
        ..Slicing::default()
    };
    slicing.resin_id = Some("grey".to_owned());
    slicing.set_material(measured_on_both());

    let saturn = slicing.catalogue.printer(SATURN).expect("shipped");
    let (profile, id) = (saturn.profile.clone(), saturn.id.clone());
    slicing.set_printer(profile, Some(id));
    let fast = slicing.material.exposure_s;

    let mars_three = slicing.catalogue.printer(MARS).expect("shipped");
    let (profile, id) = (mars_three.profile.clone(), mars_three.id.clone());
    slicing.set_printer(profile, Some(id));

    assert!(
        slicing.material.exposure_s > fast,
        "an older LED matrix needs longer than a COB engine for the same resin"
    );
    assert_eq!(slicing.resin_id.as_deref(), Some("grey"));
}

#[test]
fn a_printer_the_resin_is_not_set_up_on_brings_its_own() {
    let mut slicing = Slicing {
        catalogue: with_a_measured_resin("its-own"),
        ..Slicing::default()
    };
    // A second resin the user never measured on the Mars, held in hand.
    let mut only_saturn = measured_on_both();
    only_saturn.printers.remove(MARS);
    slicing.resin_id = Some("saturn-only".to_owned());
    slicing
        .catalogue
        .save_resin("saturn-only", &only_saturn)
        .expect("writable");
    slicing.set_material(only_saturn);

    let mars = slicing.catalogue.printer(MARS).expect("shipped");
    let (profile, id) = (mars.profile.clone(), mars.id.clone());
    slicing.set_printer(profile, Some(id));
    assert_eq!(
        slicing.resin_id.as_deref(),
        Some("grey"),
        "a resin of the Mars' own replaces one it has none of"
    );
    assert!(slicing.resin_is_tuned());
}

/// A machine with no resin of its own leaves the plate with none rather than with one
/// measured on another machine.
#[test]
fn a_printer_with_no_resin_of_its_own_leaves_none_in_hand() {
    let dir = std::env::temp_dir().join(format!("encrust-slicing-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut slicing = Slicing {
        catalogue: Catalogue::with_root(&dir).expect("a missing directory reads as empty"),
        ..Slicing::default()
    };
    let mut only_saturn = measured_on_both();
    only_saturn.printers.remove(MARS);
    slicing
        .catalogue
        .save_resin("saturn-only", &only_saturn)
        .expect("writable");
    slicing.resin_id = Some("saturn-only".to_owned());
    slicing.set_material(only_saturn);

    let mars = slicing.catalogue.printer(MARS).expect("shipped");
    let (profile, id) = (mars.profile.clone(), mars.id.clone());
    slicing.set_printer(profile, Some(id));

    assert!(slicing.resin_id.is_none());
    assert_eq!(slicing.resin_name(), "Select a resin");
}

/// An exposure is measured, so a machine alone is not enough to cut with (ADR 0196).
#[test]
fn slicing_without_a_resin_is_blocked() {
    let mut slicing = Slicing {
        printer: Some(
            PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
                .expect("the inline profile is valid"),
        ),
        ..Slicing::default()
    };
    let scene = scene_with_a_model();
    assert!(!slicing.has_resin());
    assert!(slicing.blocker(&scene).is_some());
    assert!(slicing.start(&scene, 0, PathBuf::from("out.goo")).is_err());
    assert!(slicing.job.is_none(), "a rejected start runs nothing");
}

/// What the picker reads before a resin is chosen: not having one is a state.
#[test]
fn the_resin_picker_asks_for_one_until_there_is_one() {
    let mut slicing = Slicing::default();
    assert_eq!(slicing.resin_name(), "Select a resin");
    slicing.set_material(MaterialProfile {
        name: "Grey".to_owned(),
        ..MaterialProfile::default()
    });
    assert_eq!(slicing.resin_name(), "Grey");
}

/// A resin deleted in Settings is not the resin in hand any anymore.
#[test]
fn a_resin_that_is_gone_leaves_the_window_without_one() {
    let mut slicing = Slicing {
        catalogue: with_a_measured_resin("deleted"),
        ..Slicing::default()
    };
    slicing.resin_id = Some("grey".to_owned());
    slicing.set_material(measured_on_both());
    assert!(slicing.has_resin());

    slicing
        .catalogue
        .forget_user_copy(printer_profiles::Kind::Resin, "grey")
        .expect("the user's own resin goes away");
    slicing.reload_resin();

    assert!(slicing.resin_id.is_none());
    assert!(!slicing.has_resin(), "and there is nothing to slice with");
}

#[test]
fn a_printer_opened_from_a_file_leaves_the_resin_untuned() {
    let mut slicing = Slicing::default();
    let printer =
        PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml")).expect("valid profile");

    slicing.set_printer(printer, None);

    assert!(slicing.printer_id.is_none());
    assert!(
        !slicing.resin_is_tuned(),
        "no id, so nothing to tune against"
    );
}
