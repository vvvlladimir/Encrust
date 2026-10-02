//! A branch and the trunk it joins are two overlapping solids, not one welded shape.
//! What has to hold is that the printer sees their union: no double exposure where they
//! cross, and no seam where they meet. See `docs/decisions/0041`.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Mesh, Transform, Vec3};
use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
use core_slicer::{Contour, Layer, PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{Placed, Profiles, SupportPoint, SupportTree, columns, grow, mesh_trees};
use printer_profiles::SupportProfile;

const LAYER_HEIGHT_MM: f32 = 0.05;

/// A panel fine enough that one pixel is a thirtieth of the profile's own pillar.
const PITCH_MM: f32 = 0.04;
const PANEL_PX: u32 = 1024;

/// Two tips four millimetres apart, high enough to have room to merge.
fn pair(profile: &SupportProfile) -> Vec<SupportTree> {
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 30.0)),
        SupportPoint::new(Vec3::new(14.0, 10.0, 30.0)),
    ];
    let columns = columns(
        &points,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(profile),
    );
    grow(
        &columns,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(profile),
    )
}

/// The pair welded into one mesh. Nothing here has a model to keep clear of.
fn meshed(trees: &[SupportTree], profile: &SupportProfile) -> Mesh {
    let model = Mesh::default();
    mesh_trees(
        trees,
        &Placed::new(&model, &Bvh::build(&model), Transform::default()),
        Profiles::single(profile),
    )
}

fn slice(mesh: &Mesh) -> core_slicer::Sliced {
    PlaneSliceEngine
        .slice(
            mesh,
            &SliceSettings {
                layer_height: LAYER_HEIGHT_MM,
                ..SliceSettings::default()
            },
        )
        .expect("closed tubes slice")
}

/// The area the printer actually exposes on `layer`, in square millimetres.
fn exposed_mm2(layer: &Layer) -> f32 {
    let settings = RasterSettings {
        width_px: PANEL_PX,
        height_px: PANEL_PX,
        pitch: PixelPitch {
            x: PITCH_MM,
            y: PITCH_MM,
        },
        mirror_x: false,
        mirror_y: false,
        shading: Shading::Coverage,
        grey: Grey::default(),
        blur_px: 0,
    };
    let rastered = ScanlineRasterizer
        .rasterize(layer, &settings)
        .expect("the pair sits well inside the panel");

    let lit: f64 = rastered
        .runs
        .to_mask()
        .pixels()
        .iter()
        .map(|&value| f64::from(value) / 255.0)
        .sum();
    (lit * f64::from(PITCH_MM * PITCH_MM)) as f32
}

/// The area the contours add up to, which counts anywhere two solids overlap twice.
fn contour_mm2(layer: &Layer) -> f32 {
    layer.contours.iter().map(Contour::area).sum()
}

/// The layer nearest `z` in the stack.
fn layer_at(sliced: &core_slicer::Sliced, z: f32) -> Layer {
    sliced
        .layers
        .iter()
        .min_by(|a, b| (a.z - z).abs().total_cmp(&(b.z - z).abs()))
        .expect("the stack is not empty")
        .clone()
}

#[test]
fn a_trunk_under_a_joint_exposes_one_circle_and_no_hole() {
    let profile = SupportProfile::medium();
    let trees = pair(&profile);
    assert_eq!(trees.len(), 1, "the pair merges");

    let joint = trees[0].root();
    let sliced = slice(&meshed(&trees, &profile));
    // A whole trunk diameter below the joint, past where the branch stubs reach.
    let layer = layer_at(&sliced, joint.position.z - 2.0 * joint.radius_mm);

    let expected = std::f32::consts::PI * joint.radius_mm * joint.radius_mm;
    let exposed = exposed_mm2(&layer);
    assert!(
        (exposed - expected).abs() / expected < 0.05,
        "under the joint the printer must expose one trunk of {expected} mm2, got {exposed}"
    );
}

#[test]
fn the_overlap_at_a_joint_is_exposed_once_not_twice() {
    let profile = SupportProfile::medium();
    let trees = pair(&profile);
    let joint = trees[0].root();
    let sliced = slice(&meshed(&trees, &profile));

    // Half a trunk radius under the joint, where both stubs still overlap the trunk.
    let layer = layer_at(&sliced, joint.position.z - joint.radius_mm / 2.0);
    let exposed = exposed_mm2(&layer);
    let summed = contour_mm2(&layer);

    assert!(
        layer.contours.len() > 1,
        "the joint is where the stubs and the trunk are separate solids"
    );
    assert!(
        exposed < summed * 0.95,
        "the non-zero winding fill must union the overlap, not add it up: {exposed} mm2 \
         exposed against {summed} mm2 of contour"
    );
    assert!(
        exposed > std::f32::consts::PI * joint.radius_mm * joint.radius_mm * 0.95,
        "the union is at least the trunk it contains, got {exposed} mm2"
    );
}

#[test]
fn every_layer_of_a_branched_support_is_solid() {
    let profile = SupportProfile::medium();
    let trees = pair(&profile);
    let sliced = slice(&meshed(&trees, &profile));

    for layer in &sliced.layers {
        assert!(
            !layer.contours.is_empty(),
            "the support cures on every layer it passes through, {} is empty",
            layer.z
        );
        assert!(
            exposed_mm2(layer) > 0.0,
            "a layer with contours has to light pixels, {} does not",
            layer.z
        );
    }
}
