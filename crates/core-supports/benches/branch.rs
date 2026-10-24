//! What merging tips into trunks costs, and what it saves; the saving is its own benchmark so a
//! change to the merge rule shows up as a number.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, signed_volume};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{Placed, Profiles, SupportPoint, columns, generate_supports, grow, mesh_trees};
use criterion::{Criterion, criterion_group, criterion_main};
use printer_profiles::SupportProfile;

/// The layer height the shipped resin profile prints at.
const LAYER_HEIGHT_MM: Scalar = 0.05;

/// A ball `lift` mm clear of the plate, as a resin part is laid out: its lower cap is filled with
/// the tip lattice branching thins out, and the lift is the room trunks merge in.
fn ball(radius: Scalar, lift: Scalar, segments: u32, rings: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        for segment in 0..segments {
            let theta = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            vertices.push(Vec3::new(
                radius * phi.sin() * theta.cos(),
                radius * phi.sin() * theta.sin(),
                lift + radius - radius * phi.cos(),
            ));
        }
    }
    for ring in 0..rings {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let low_left = ring * segments + segment;
            let low_right = ring * segments + next;
            let high_left = (ring + 1) * segments + segment;
            let high_right = (ring + 1) * segments + next;
            faces.push([low_left, high_right, high_left]);
            faces.push([low_left, low_right, high_right]);
        }
    }
    Mesh::new(vertices, faces)
}

/// The tips automatic placement puts under `model`, already dropped onto their landings.
fn tips_under(model: &Mesh, bvh: &Bvh, profile: &SupportProfile) -> Vec<core_supports::Column> {
    let sliced = PlaneSliceEngine
        .slice(
            model,
            &SliceSettings {
                layer_height: LAYER_HEIGHT_MM,
                ..SliceSettings::default()
            },
        )
        .expect("a closed ball slices");
    let points: Vec<SupportPoint> = generate_supports(
        &sliced,
        LAYER_HEIGHT_MM,
        profile,
        None,
        &[],
        &[],
        None,
        &mut |_| true,
    )
    .into_iter()
    .map(|need| SupportPoint::new(need.contact))
    .collect();
    columns(
        &points,
        &Placed::new(model, bvh, Transform::default()),
        Profiles::single(profile),
    )
}

fn branching(c: &mut Criterion) {
    let profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    let mut straight = profile.clone();
    straight.branching.enabled = false;

    let model = ball(20.0, 10.0, 256, 128);
    let bvh = Bvh::build(&model);
    let tips = tips_under(&model, &bvh, &profile);

    c.bench_function("grow/40 mm ball, lifted", |b| {
        b.iter(|| {
            grow(
                &tips,
                &Placed::new(&model, &bvh, Transform::default()),
                Profiles::single(&profile),
            )
        });
    });

    // Standing on the plate, the outer pads stand beside it, so each looks round at a
    // part rather than at an empty plate.
    let seated = ball(20.0, 0.0, 256, 128);
    let seated_bvh = Bvh::build(&seated);
    let seated_tips = tips_under(&seated, &seated_bvh, &profile);
    c.bench_function("grow/40 mm ball, on the plate", |b| {
        b.iter(|| {
            grow(
                &seated_tips,
                &Placed::new(&seated, &seated_bvh, Transform::default()),
                Profiles::single(&profile),
            )
        });
    });

    let merged = grow(
        &tips,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&profile),
    );
    let apart = grow(
        &tips,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&straight),
    );
    let merged_mm3 = signed_volume(&mesh_trees(
        &merged,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&profile),
    ));
    let apart_mm3 = signed_volume(&mesh_trees(
        &apart,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&straight),
    ));
    println!(
        "branching/40 mm ball, lifted: {} tips on {} trunks, {merged_mm3:.0} mm3 against \
         {apart_mm3:.0} mm3, {:.0}% saved",
        tips.len(),
        merged.len(),
        100.0 * (1.0 - merged_mm3 / apart_mm3),
    );

    c.bench_function("mesh_trees/40 mm ball, lifted", |b| {
        b.iter(|| {
            mesh_trees(
                &merged,
                &Placed::new(&model, &bvh, Transform::default()),
                Profiles::single(&profile),
            )
        });
    });
}

criterion_group!(benches, branching);
criterion_main!(benches);
