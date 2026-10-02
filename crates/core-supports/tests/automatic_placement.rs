//! What automatic placement hands over has to be printable: every point it returns must
//! become a column that stands, and those columns must mesh into closed geometry the
//! slicer can cut.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Mesh, Transform, Vec3, Winding, diagnose, signed_volume};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{
    Placed, Profiles, SupportPoint, SupportTree, columns, generate_supports, grow, mesh_trees,
};
use printer_profiles::SupportProfile;

const LAYER_HEIGHT_MM: f32 = 0.05;

/// An axis-aligned box spanning `min`..`max`, twelve triangles wound outwards.
fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
    let vertices = vec![
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    Mesh::new(vertices, faces)
}

/// A 20 mm box hanging 10 mm over the plate: nothing holds it up but supports.
fn floating_box() -> Mesh {
    box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(20.0, 20.0, 30.0))
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
        .expect("a closed box slices")
}

/// The same profile with every tip standing on a column of its own and nothing tying the
/// columns together, which is what the measurements of a single pillar are made against.
fn unbranched() -> SupportProfile {
    let mut profile = SupportProfile::medium();
    profile.branching.enabled = false;
    profile.bracing.enabled = false;
    profile
}

/// The forest automatic placement grows under `mesh`.
fn forest(mesh: &Mesh, profile: &SupportProfile) -> Vec<SupportTree> {
    let bvh = Bvh::build(mesh);
    let standing = Placed::new(mesh, &bvh, Transform::default());
    let columns = columns(&points(mesh, profile), &standing, Profiles::single(profile));
    grow(&columns, &standing, Profiles::single(profile))
}

fn points(mesh: &Mesh, profile: &SupportProfile) -> Vec<SupportPoint> {
    generate_supports(
        &slice(mesh),
        LAYER_HEIGHT_MM,
        profile,
        &[],
        None,
        &mut |_| true,
    )
    .into_iter()
    .map(SupportPoint::new)
    .collect()
}

/// The forest welded into one mesh, with `model` in the way of anything that has to
/// keep clear of it.
fn meshed(trees: &[SupportTree], model: &Mesh, profile: &SupportProfile) -> Mesh {
    mesh_trees(
        trees,
        &Placed::new(model, &Bvh::build(model), Transform::default()),
        Profiles::single(profile),
    )
}

#[test]
fn every_point_placed_under_an_island_becomes_a_column_that_stands() {
    let profile = SupportProfile::medium();
    let model = floating_box();
    let points = points(&model, &profile);
    assert!(!points.is_empty(), "an island has to be held up");

    let columns = columns(
        &points,
        &Placed::new(&model, &Bvh::build(&model), Transform::default()),
        Profiles::single(&profile),
    );
    assert_eq!(
        columns.len(),
        points.len(),
        "every point had room for a column"
    );
    for column in &columns {
        let landing = column.landing.expect("every contact had room for a column");
        assert!(
            !landing.on_model,
            "there is nothing but plate under the island"
        );
        assert!(
            landing.base.z.abs() < 1e-5,
            "the column reaches the plate, got {}",
            landing.base.z
        );
        assert!(
            (column.contact.z - 10.0).abs() < LAYER_HEIGHT_MM,
            "the contact is on the underside of the island, got {}",
            column.contact.z
        );
    }
}

#[test]
fn the_columns_of_a_run_mesh_into_a_closed_solid() {
    let profile = SupportProfile::medium();
    let model = floating_box();

    let mesh = meshed(&forest(&model, &profile), &model, &profile);
    assert!(!mesh.is_empty());
    assert_eq!(
        diagnose(&mesh).boundary_edges,
        0,
        "an open support mesh would leave the slicer stitching over holes"
    );
}

#[test]
fn branching_holds_an_island_up_with_less_resin_than_columns_do() {
    let model = floating_box();
    let branched = SupportProfile::medium();
    let straight = unbranched();

    let merged = signed_volume(&meshed(&forest(&model, &branched), &model, &branched));
    let apart = signed_volume(&meshed(&forest(&model, &straight), &model, &straight));

    assert!(
        merged < apart,
        "trunks under a lattice of tips hold less than a pillar each: {merged} mm3 \
         against {apart} mm3"
    );
}

#[test]
fn every_tip_of_a_run_is_still_carried_after_the_tips_merge() {
    let profile = SupportProfile::medium();
    let model = floating_box();
    let bvh = Bvh::build(&model);
    let columns = columns(
        &points(&model, &profile),
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&profile),
    );

    let forest = grow(
        &columns,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&profile),
    );
    let carried: usize = forest.iter().map(SupportTree::tip_count).sum();
    assert_eq!(
        carried,
        columns.len(),
        "merging moves tips onto shared trunks, it never drops one"
    );
    assert!(
        forest.len() < columns.len(),
        "a lattice of {} tips over one island has to share some trunks, got {} of them",
        columns.len(),
        forest.len()
    );
}

#[test]
fn the_layer_under_an_island_exposes_the_supports_that_hold_it() {
    // Measured without branching: one tip is one pillar, so the exposed area is the
    // profile's own cross-section times the number of them.
    let profile = unbranched();
    let model = floating_box();
    let columns = columns(
        &points(&model, &profile),
        &Placed::new(&model, &Bvh::build(&model), Transform::default()),
        Profiles::single(&profile),
    );
    let supports = meshed(
        &grow(
            &columns,
            &Placed::new(&model, &Bvh::build(&model), Transform::default()),
            Profiles::single(&profile),
        ),
        &model,
        &profile,
    );

    // Five millimetres up is clear air under the model, so everything on that layer is
    // pillar: one ring of the profile's own diameter per column.
    let layer = slice(&supports)
        .layers
        .into_iter()
        .find(|layer| (layer.z - 5.0).abs() < LAYER_HEIGHT_MM)
        .expect("the columns reach half way down");

    let area: f32 = layer.contours.iter().map(core_slicer::Contour::area).sum();
    let radius = profile.pillar_radius_mm();
    let expected = columns.len() as f32 * std::f32::consts::PI * radius * radius;
    assert!(
        (area - expected).abs() / expected < 0.05,
        "expected about {expected} mm2 of pillar across {} columns, got {area}",
        columns.len()
    );
}

/// A ball of `radius` resting on the plate: the shape every slicer is judged on, because
/// its whole lower cap hangs over nothing at a different angle on every layer.
fn ball(radius: f32) -> Mesh {
    let (segments, rings) = (128u32, 64u32);
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..segments {
            let theta = std::f32::consts::TAU * segment as f32 / segments as f32;
            vertices.push(Vec3::new(
                radius * phi.sin() * theta.cos(),
                radius * phi.sin() * theta.sin(),
                radius - radius * phi.cos(),
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

#[test]
fn a_ball_on_the_plate_is_held_under_its_cap_and_nowhere_above_it() {
    let profile = SupportProfile::medium();
    let model = ball(15.0);
    let points = points(&model, &profile);

    assert!(
        points.len() >= 8,
        "a 30 mm ball hangs over nothing all round its lower cap, got {}",
        points.len()
    );

    // A ball resting on the plate leaves no room under its own skirt: a column there can
    // neither stand on the skirt, which is too steep, nor put a foot down beside it. What
    // holds those tips is the lean `grow` bends them into; see `docs/decisions/0077`.
    let bvh = Bvh::build(&model);
    let columns = columns(
        &points,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&profile),
    );
    let trees = grow(
        &columns,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&profile),
    );
    let standing: usize = trees.iter().map(SupportTree::tip_count).sum();
    assert!(
        standing >= 4,
        "the rim of the cap is far enough out for a foot, got {standing} supports"
    );
    assert!(
        trees.iter().all(|tree| !tree.landing().on_model),
        "a ball that steep has no face to stand a support on"
    );

    // The point of all of it: none of that geometry is inside the ball but the bite each
    // tip takes; see `docs/decisions/0077`.
    let mesh = meshed(&trees, &model, &profile);
    let winding = Winding::build(&model);
    let bvh = Bvh::build(&model);
    let bite_mm = profile.contact_depth_mm() + 0.05;
    for vertex in &mesh.vertices {
        if winding.at(&model, *vertex) < 0.5 {
            continue;
        }
        let depth = bvh
            .closest(&model, *vertex)
            .expect("the ball has faces")
            .distance;
        assert!(
            depth <= bite_mm,
            "{vertex} is {depth} mm inside the ball, deeper than a tip's bite"
        );
    }

    for column in &columns {
        // The ball holds itself up above the latitude where it leans 45 degrees, which
        // is its radius over root two below the equator.
        assert!(
            column.contact.z <= 15.0 - 15.0 / 2.0_f32.sqrt() + 1.0,
            "{} is above the latitude that holds itself up",
            column.contact
        );
    }
}

#[test]
fn a_denser_profile_holds_the_same_island_in_more_places() {
    let model = floating_box();
    let light = points(&model, &SupportProfile::light());
    let heavy = points(&model, &SupportProfile::heavy());
    assert!(
        heavy.len() > light.len(),
        "the heavy preset placed {} against the light preset's {}",
        heavy.len(),
        light.len()
    );
}
