//! `encrust render`: the staged plate drawn from the sides asked for, as PNG pictures.

use crate::support::*;

fn render(args: &[&str]) -> std::process::Output {
    encrust(&[&["render"], args].concat())
}

#[test]
fn every_view_asked_for_is_written_as_a_png_named_after_it_at_the_size_asked() {
    let cube = write_box_stl("render-views", 10.0, 12, 0);
    let out = output_dir("render-views");
    let output = render(&[
        cube.to_str().unwrap(),
        "--view",
        "iso,top,bottom",
        "--size",
        "64",
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
    let report = json(&output);
    assert_eq!(report["schema"], 1);
    let pictures = report["pictures"].as_array().expect("a list of pictures");
    assert_eq!(pictures.len(), 3);
    for (picture_json, view) in pictures.iter().zip(["iso", "top", "bottom"]) {
        let path = out.join(format!("{view}.png"));
        assert_eq!(picture_json["view"], view);
        assert_eq!(picture_json["path"], path.to_str().unwrap());
        let (width, height, pixels) = png_pixels(&path);
        assert_eq!((width, height), (64, 64), "{view} is 64 px square");
        assert_eq!(pixels.len(), 64 * 64 * 3, "{view} is in colour");
        assert!(
            pixels.chunks(3).any(|pixel| pixel != &pixels[..3]),
            "{view} shows something besides the background"
        );
    }
}

#[test]
fn a_png_name_takes_the_one_view_and_lists_where_each_mark_landed() {
    let cube = write_box_stl("render-one", 10.0, 12, 0);
    let out = output_file("render-one.png");
    let output = render(&[
        cube.to_str().unwrap(),
        "--view",
        "top",
        "--size",
        "100",
        "--mark",
        "5,5,10",
        "--mark",
        "5,5,0",
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(png_pixels(&out).0, 100);
    let marks = &json(&output)["pictures"][0]["markers"];
    // The middle of the cube's top is the middle of the picture; its bottom is under it.
    for (index, hidden) in [(0, false), (1, true)] {
        let mark = &marks[index];
        for axis in ["x_px", "y_px"] {
            let at = mark[axis].as_f64().expect("a pixel coordinate");
            assert!(
                (at - 50.0).abs() < 1.0,
                "expected mark {index} at 50 px, got {axis} {at}"
            );
        }
        assert_eq!(mark["hidden"], hidden, "mark {index}");
    }
}

#[test]
fn an_unknown_view_or_a_size_out_of_range_is_a_usage_error() {
    let cube = write_box_stl("render-usage", 10.0, 12, 0);
    for wrong in [["--view", "sideways"], ["--size", "8"]] {
        let output = render(&[cube.to_str().unwrap(), wrong[0], wrong[1]]);
        assert_eq!(output.status.code(), Some(2), "{wrong:?}");
    }
}

#[test]
fn a_strict_render_of_an_open_model_exits_3_and_draws_nothing() {
    let open = write_box_stl("render-open", 10.0, 11, 0);
    let out = output_file("render-open.png");
    let output = render(&[
        open.to_str().unwrap(),
        "--strict",
        "--view",
        "top",
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(json(&output)["pictures"], serde_json::json!([]));
    assert!(
        !out.exists(),
        "a strict run leaves nothing it would not print"
    );
}

#[test]
fn a_png_name_for_several_views_is_refused_before_anything_is_written() {
    let cube = write_box_stl("render-several", 10.0, 12, 0);
    let out = output_file("render-several.png");
    let output = render(&[
        cube.to_str().unwrap(),
        "--view",
        "front,back",
        "-o",
        out.to_str().unwrap(),
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("name a directory for several"));
    assert!(!out.exists());
}
