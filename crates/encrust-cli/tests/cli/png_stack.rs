//! The debug PNG stack: one greyscale image a layer, where it sits on the panel and
//! what shading and blur do to its edges.

use crate::support::*;

#[test]
fn a_cube_becomes_one_greyscale_png_per_layer() {
    let path = write_box_stl("raster-cube", 10.0, 12, 0);
    let out = output_dir("cube");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "panel"), "200 x 200 px at 0.1000 x 0.1000 mm");
    assert_eq!(
        field(&text, "shading"),
        "coverage, exact pixel area, all 255 greys down to 128",
        "the floor comes from the profile without being asked for"
    );
    assert_eq!(
        field(&text, "masks"),
        format!("10 written to {}", out.display())
    );
    assert!(!text.contains("raster defect"));
    assert_eq!(written_layers(&out), 10);

    let (width, height, pixels) = layer_pixels(&out, 5);
    assert_eq!((width, height), (200, 200));
    assert_eq!(
        pixels.iter().filter(|&&p| p == 255).count(),
        100 * 100,
        "a 10 mm cube at a 0.1 mm pitch exposes 100 x 100 whole pixels"
    );
}

#[test]
fn the_stack_sits_where_the_model_sits_on_the_plate() {
    let path = write_box_stl("raster-corner", 10.0, 12, 0);
    let out = output_dir("corner");
    slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "-o",
        out.to_str().unwrap(),
    ]);

    // The cube sits at the plate origin, which is the first row the file carries.
    let (_, _, pixels) = layer_pixels(&out, 0);
    assert_eq!(
        pixels[0], 255,
        "the first pixel of the first row is exposed"
    );
    assert_eq!(pixels[199 * 200], 0, "the last row is not");
}

#[test]
fn rounding_and_the_floor_are_asked_for_on_the_command_line() {
    let path = write_box_stl("raster-rungs", 10.0, 12, 0);
    let out = output_dir("rungs");
    let text = stdout(&slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--grey-levels",
        "4",
        "--grey-floor",
        "0",
        "--rotate",
        "0,0,20",
        "-o",
        out.to_str().unwrap(),
    ]));

    assert_eq!(
        field(&text, "shading"),
        "coverage, exact pixel area, 4 greys"
    );
    let (_, _, pixels) = layer_pixels(&out, 0);
    let rungs = [0, 63, 127, 191, 255];
    assert!(
        pixels.iter().all(|p| rungs.contains(p)),
        "four levels leave only their own rungs"
    );
}

#[test]
fn binary_shading_writes_no_intermediate_grey() {
    let path = write_box_stl("raster-binary", 10.0, 12, 0);
    let out = output_dir("binary");
    let text = stdout(&slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--no-anti-alias",
        "--rotate",
        "0,0,20",
        "-o",
        out.to_str().unwrap(),
    ]));

    assert_eq!(field(&text, "shading"), "binary, no intermediate grey");
    let (_, _, pixels) = layer_pixels(&out, 0);
    assert!(
        pixels.iter().all(|&p| p == 0 || p == 255),
        "a turned cube has diagonal edges, and none of them may come out grey"
    );
}

#[test]
fn a_blur_widens_the_grey_at_every_edge() {
    let path = write_box_stl("raster-blur", 10.0, 12, 0);
    let grey_pixels = |name: &str, blur: &str| {
        let out = output_dir(name);
        let text = stdout(&slice(&[
            path.to_str().unwrap(),
            "--profile",
            test_panel().to_str().unwrap(),
            "--layer-height",
            "5",
            "--grey-floor",
            "0",
            "--blur",
            blur,
            "-o",
            out.to_str().unwrap(),
        ]));
        let (_, _, pixels) = layer_pixels(&out, 0);
        let grey = pixels.iter().filter(|&&p| p != 0 && p != 255).count();
        (text, grey)
    };

    let (_, sharp) = grey_pixels("blur-off", "0");
    let (text, blurred) = grey_pixels("blur-on", "2");
    assert!(field(&text, "shading").ends_with("blurred 2 px"), "{text}");
    assert!(
        blurred > sharp + 4 * 10,
        "a radius of two greys several more pixels along each side: {sharp} -> {blurred}"
    );
}

#[test]
fn a_model_hanging_off_the_display_is_clipped_and_fails_strict() {
    // The fixture panel is 20 mm wide and its build volume is 40, so this model passes
    // the millimetre fit check and still runs off the display.
    let path = write_box_stl("raster-oversized", 30.0, 12, 0);
    let out = output_dir("clipped");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
        "--strict",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);

    assert_eq!(field(&text, "fits"), "Test panel: yes");
    assert_eq!(
        field(&text, "raster defect"),
        "6 layers clipped, up to 100.0 px past the panel"
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "--strict must fail on a layer that does not fit the display"
    );
    assert!(
        !out.exists(),
        "--strict keeps nothing: code 3 and a stack on disk would contradict each other"
    );
    assert!(text.contains("was not kept"), "{text}");
}

#[test]
fn without_a_profile_the_panel_is_unknown_and_no_masks_are_written() {
    let path = write_box_stl("no-profile", 10.0, 12, 0);
    let out = output_dir("no-profile");
    let output = slice(&[
        path.to_str().unwrap(),
        "--layer-height",
        "5",
        "-o",
        out.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "the reports are still worth having"
    );
    assert!(!stdout(&output).contains("masks"));
    assert!(!out.exists());
}
