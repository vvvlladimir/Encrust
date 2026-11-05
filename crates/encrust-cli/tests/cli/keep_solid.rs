//! Keeping a shell's wall solid from the command line, as the window's Keep solid clicks do.

use crate::hollowing::hollow_cube;
use crate::support::*;

/// The resin a 16 mm cube with a 3 mm wall and `extra` saves, cubic millimetres.
fn saved_mm3(name: &str, extra: &[&str]) -> f32 {
    let mut args = vec!["--hollow", "3"];
    args.extend_from_slice(extra);
    let (_, text) = hollow_cube(name, &args);
    field(&text, "resin saved")
        .trim_end_matches(" mm^3")
        .parse()
        .expect("the saved volume is a number")
}

#[test]
fn a_blocker_of_a_set_size_reaches_that_far_past_the_wall() {
    let open = saved_mm3("solid-open", &[]);
    let kept = saved_mm3(
        "solid-sized",
        &["--keep-solid", "8,8,16", "--keep-solid-size", "3"],
    );

    // A ball of 3 + 3 about the top's middle dips 3 mm into the cavity under the 3 mm lid:
    // a cap of pi h^2 (3r - h) / 3 = 141 mm3.
    let expected = std::f32::consts::PI * 9.0 * (18.0 - 3.0) / 3.0;
    let taken = open - kept;
    assert!(
        (taken - expected).abs() / expected < 0.25,
        "the blocker keeps a cap of {expected} mm3 solid, got {taken}"
    );
}

#[test]
fn a_fitted_blocker_fills_the_cube_it_lands_on() {
    let kept = saved_mm3("solid-fitted", &["--keep-solid", "8,8,16"]);

    assert!(
        kept < 10.0,
        "a ball as wide as the cube, walled, takes the whole 10 mm cavity, left {kept}"
    );
}

#[test]
fn points_run_together_lay_a_rod_between_them() {
    let open = saved_mm3("solid-rod-open", &[]);
    let rod = saved_mm3(
        "solid-rod",
        &["--keep-solid", "3,8,16:13,8,16", "--keep-solid-size", "3"],
    );

    // A rod of 6 along the lid from x = 3 to 13, its axis 3 mm over the cavity: a circular
    // segment of r^2 acos(d / r) - d sqrt(r^2 - d^2) = 22.1 mm2 down 10 mm of it.
    let segment = 36.0 * 0.5_f32.acos() - 3.0 * 27.0_f32.sqrt();
    let expected = segment * 10.0;
    let taken = open - rod;
    assert!(
        (taken - expected).abs() / expected < 0.25,
        "the rod keeps {expected} mm3 solid, got {taken}"
    );
}

#[test]
fn a_cube_thinner_than_the_thickness_kept_solid_gets_no_cavity() {
    let open = saved_mm3("solid-under-open", &[]);
    let under = saved_mm3("solid-under-thin", &["--keep-solid-under", "4"]);
    let thick = saved_mm3("solid-under-thick", &["--keep-solid-under", "20"]);

    // The 10 mm cavity of the 16 mm cube is 1000 mm3, and nothing of it is under 4 mm.
    assert!(
        (under - open).abs() < open * 0.02,
        "nothing is under 4 mm, so {open} mm3 saved, got {under}"
    );
    assert!(
        thick == 0.0,
        "a 16 mm cube is under 20 mm, so 0 mm3 saved, got {thick}"
    );
}

#[test]
fn keeping_solid_with_no_shell_or_a_size_with_no_point_is_refused() {
    let path = write_box_stl("solid-refused", 16.0, 12, 0);
    let out = output_dir("solid-refused");
    let panel = test_panel();
    for (flags, named) in [
        (&["--keep-solid", "8,8,16"][..], "--keep-solid"),
        (&["--keep-solid-under", "4"][..], "--keep-solid-under"),
        (
            &["--hollow", "3", "--keep-solid-under", "0"][..],
            "--keep-solid-under",
        ),
        (
            &["--hollow", "3", "--keep-solid-size", "2"][..],
            "--keep-solid-size",
        ),
        (
            &[
                "--hollow",
                "3",
                "--keep-solid",
                "8,8,16",
                "--keep-solid-size",
                "0",
            ][..],
            "--keep-solid-size",
        ),
    ] {
        let mut args = vec![
            path.to_str().unwrap(),
            "--profile",
            panel.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ];
        args.extend_from_slice(flags);
        let output = slice(&args);
        assert!(!output.status.success(), "{flags:?} is refused");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(named),
            "and the refusal names {named}"
        );
    }
}
