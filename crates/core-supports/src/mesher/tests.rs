use super::foot::{FLARE_RINGS, FOOT_BEVEL, FOOT_BITE, flare, foot, trunk_base_mm, trunk_bottom};
use super::*;
use crate::Landing;
use crate::SupportPoint;
use crate::fixtures::{box_mesh, meshed, profile, ramp, trees_on};
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::{SupportTree, TreeNode};
use core_geometry::{Transform, Vec2, diagnose, signed_volume};
use printer_profiles::{ContactShape, SupportProfile};
use std::f32::consts::PI;

/// One column standing on the plate under a contact `height` millimetres up, with
/// nothing for it to merge with.
fn column_on_plate(height: Scalar, profile: &SupportProfile) -> Vec<SupportTree> {
    trees_on(
        &[SupportPoint::new(Vec3::new(10.0, 10.0, height))],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(profile),
    )
}

#[test]
fn a_pad_cut_back_by_a_kerb_is_closed_and_stops_short_of_it() {
    // A kerb 2 mm tall with its face at x = 1, and a trunk 1.9 mm off it: too near to
    // move the pad aside, so it is cut back to keep its 0.5 mm of air.
    let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
    let profile = profile();
    let trees = trees_on(
        &[SupportPoint::new(Vec3::new(2.9, 0.0, 20.0))],
        &kerb,
        Transform::default(),
        Profiles::single(&profile),
    );
    assert!(trees[0].landing().pad_cut.is_some(), "the pad is cut");
    let mesh = meshed(&trees, Profiles::single(&profile));
    let diagnostics = diagnose(&mesh);
    assert_eq!(diagnostics.boundary_edges, 0, "a cut pad has no open edges");
    assert_eq!(diagnostics.non_manifold_edges, 0);
    let nearest = mesh
        .vertices
        .iter()
        .map(|at| at.x)
        .fold(Scalar::INFINITY, Scalar::min);
    assert!(
        nearest >= 1.5 - 1e-3,
        "nothing of it comes within the clearance of the kerb, got x = {nearest}"
    );
}

#[test]
fn a_column_is_closed_and_wound_outwards() {
    let profile = profile();
    let mesh = meshed(&column_on_plate(20.0, &profile), Profiles::single(&profile));

    let diagnostics = diagnose(&mesh);
    assert_eq!(
        diagnostics.boundary_edges, 0,
        "a printable column has no open edges"
    );
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.degenerate_faces, 0);
    assert!(
        signed_volume(&mesh) > 0.0,
        "outward winding gives a positive volume"
    );
}

#[test]
fn each_group_is_meshed_apart_and_together_they_are_the_whole() {
    let profiles = [
        profile(),
        SupportProfile::shipped("heavy").expect("the heavy preset ships"),
    ];
    let profiles = Profiles::new(&profiles).expect("two groups");
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(40.0, 10.0, 20.0)).in_group(1),
    ];
    let model = Mesh::default();
    let bvh = core_geometry::Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let trees = trees_on(&points, &model, Transform::default(), profiles);

    let groups = mesh_groups(&trees, &placed, profiles);
    let whole = mesh_trees(&trees, &placed, profiles);
    assert_eq!(groups.len(), 2);
    assert!(
        groups.iter().all(|mesh| !mesh.is_empty()),
        "one column each"
    );
    assert_eq!(
        groups.iter().map(|mesh| mesh.faces.len()).sum::<usize>(),
        whole.faces.len(),
        "split by group, nothing added and nothing lost"
    );
}

#[test]
fn a_brace_is_meshed_with_the_group_it_ties() {
    // Two tall trunks of the second group, close enough to tie, and nothing at all
    // in the first.
    let mut braced = profile();
    braced.branching.enabled = false;
    braced.bracing = printer_profiles::Bracing {
        enabled: true,
        start_height_mm: 5.0,
        rise_mm: 5.0,
        max_spacing_mm: 12.0,
        ..printer_profiles::Bracing::default()
    };
    let table = [profile(), braced];
    let profiles = Profiles::new(&table).expect("two groups");
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 30.0)).in_group(1),
        SupportPoint::new(Vec3::new(18.0, 10.0, 30.0)).in_group(1),
    ];
    let model = Mesh::default();
    let bvh = core_geometry::Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let trees = trees_on(&points, &model, Transform::default(), profiles);

    let groups = mesh_groups(&trees, &placed, profiles);
    assert!(
        groups[0].is_empty(),
        "the first group holds nothing, so nothing is drawn in its colour"
    );
    let between = groups[1]
        .vertices
        .iter()
        .filter(|vertex| vertex.x > 12.0 && vertex.x < 16.0)
        .count();
    assert!(
        between > 0,
        "the braces between the two trunks are the second group's"
    );
}

/// A support by hand under a slab's underside at z = 10: a head straight down from
/// the contact at the origin to a neck 2 mm under it, and a trunk to the plate.
fn under_a_slab(beside: &[Mesh]) -> (Mesh, SupportTree) {
    let mut model = box_mesh(Vec3::new(-10.0, -10.0, 10.0), Vec3::new(10.0, 10.0, 12.0));
    for other in beside {
        let offset = model.vertices.len() as u32;
        model.vertices.extend_from_slice(&other.vertices);
        model
            .faces
            .extend(other.faces.iter().map(|face| face.map(|at| at + offset)));
    }
    let radius_mm = profile().pillar_radius_mm();
    let nodes = vec![
        TreeNode {
            position: Vec3::new(0.0, 0.0, 10.0),
            radius_mm,
            parent: Some(1),
            point: Some(0),
            head: None,
        },
        TreeNode {
            position: Vec3::new(0.0, 0.0, 8.0),
            radius_mm,
            parent: None,
            point: None,
            head: None,
        },
    ];
    let landing = Landing {
        base: Vec3::ZERO,
        on_model: false,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    (model, SupportTree::new(nodes, 1, landing))
}

fn is_sound(model: &Mesh, tree: &SupportTree) -> bool {
    let bvh = core_geometry::Bvh::build(model);
    let fixture = profile();
    sound(
        tree,
        Profiles::single(&fixture),
        &Placed::new(model, &bvh, Transform::default()),
    )
}

#[test]
fn a_support_clear_of_the_model_but_for_its_tip_is_sound() {
    let (model, tree) = under_a_slab(&[]);
    assert!(is_sound(&model, &tree));
}

#[test]
fn a_head_a_thin_rod_pokes_into_is_not_sound() {
    // A rod 0.1 mm square hanging 1.5 mm from the underside, 0.3 mm off the head's
    // axis, where the head is 0.4 mm in radius halfway down.
    let rod = box_mesh(Vec3::new(0.25, -0.05, 8.5), Vec3::new(0.35, 0.05, 10.0));
    let (model, tree) = under_a_slab(&[rod]);
    assert!(!is_sound(&model, &tree));
}

#[test]
fn a_trunk_a_ledge_reaches_into_is_not_sound() {
    // A ledge reaching 0.2 mm inside the 0.6 mm pillar, 4 mm over the plate.
    let ledge = box_mesh(Vec3::new(0.4, -5.0, 3.5), Vec3::new(6.0, 5.0, 4.0));
    let (model, tree) = under_a_slab(&[ledge]);
    assert!(!is_sound(&model, &tree));
}

#[test]
fn a_head_lying_along_its_face_is_not_sound() {
    // The neck slid 2 mm aside at the contact's own height: the head runs along the
    // underside rather than leaving it.
    let (model, mut tree) = under_a_slab(&[]);
    tree.move_node(1, Vec3::new(2.0, 0.0, 9.9));
    assert!(!is_sound(&model, &tree));
}

#[test]
fn a_column_landing_on_the_model_is_closed_too() {
    let profile = profile();
    let cube = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let trees = trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 5.0, 25.0))],
        &cube,
        Transform::default(),
        Profiles::single(&profile),
    );
    assert!(trees[0].landing().on_model);

    let diagnostics = diagnose(&meshed(&trees, Profiles::single(&profile)));
    assert_eq!(diagnostics.boundary_edges, 0);
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.degenerate_faces, 0);
}

#[test]
fn a_column_shorter_than_its_neck_is_still_closed() {
    let profile = profile();
    // Barely taller than the foot, so the neck is squeezed down to nothing.
    let trees = column_on_plate(profile.bottom.platform_thickness_mm + 0.3, &profile);
    assert_eq!(trees.len(), 1);

    let diagnostics = diagnose(&meshed(&trees, Profiles::single(&profile)));
    assert_eq!(diagnostics.boundary_edges, 0);
    assert_eq!(diagnostics.degenerate_faces, 0);
}

#[test]
fn the_volume_matches_the_cones_and_cylinders_it_is_made_of() {
    let profile = SupportProfile {
        // A round number of facets does not change the analysis, but many of them
        // brings the polygon's area close to the circle's.
        facets: 512,
        ..profile()
    };
    let height = 20.0;
    let mesh = meshed(
        &column_on_plate(height, &profile),
        Profiles::single(&profile),
    );

    let contact_r = profile.contact_radius_mm();
    let pillar_r = profile.pillar_radius_mm();
    let base_r = profile.base_radius_mm();
    let foot_h = profile.bottom.platform_thickness_mm;
    let bevel = foot_h * FOOT_BEVEL;
    let rim_r = base_r - bevel;
    let trunk_bottom = profile.bottom.platform_thickness_mm - foot_h * FOOT_BITE;

    // Bite, neck, pillar, flare and pad frustum; pillar and flare overlap the foot as solids of
    // their own and count twice (ADR 0041); a plane tip's bite lengthens the pillar.
    let tip = PI * pillar_r * pillar_r * profile.tip.contact_depth_mm;
    let neck = PI * profile.top.length_mm / 3.0
        * (contact_r * contact_r + contact_r * pillar_r + pillar_r * pillar_r);
    // The neck ends in a dome inside the pillar and the pillar starts under one inside the neck,
    // each a cone on its pole and two frustums, counted on top of the tube they sit in.
    let pillar = PI * pillar_r * pillar_r * (height - profile.top.length_mm - trunk_bottom);
    let rings = tube::DOME_RINGS as f32;
    let ring = |step: f32| (PI / 2.0 * step / rings).sin() * pillar_r;
    let drop = |step: f32| (1.0 - (PI / 2.0 * step / rings).cos()) * pillar_r;
    let dome: f32 = (0..tube::DOME_RINGS)
        .map(|step| step as f32)
        .map(|step| {
            let (upper, lower) = (ring(step), ring(step + 1.0));
            PI * (drop(step + 1.0) - drop(step)) / 3.0
                * (upper * upper + upper * lower + lower * lower)
        })
        .sum();
    let seam = 2.0 * dome;
    let pad = PI * foot_h / 3.0 * (base_r * base_r + base_r * rim_r + rim_r * rim_r);
    // The flare is a quarter circle as tall as it is wide, drawn as a frustum a step,
    // and sinks into the pad as far as the pillar does.
    let flare_bottom_r = profile.flare_lower_radius_mm();
    let rise = flare_bottom_r - pillar_r;
    let steps = FLARE_RINGS as f32;
    let at = |step: f32| {
        let angle = PI / 2.0 * step / steps;
        (rise * angle.sin(), pillar_r + rise * (1.0 - angle.cos()))
    };
    let fillet: f32 = (0..FLARE_RINGS)
        .map(|step| step as f32)
        .map(|step| {
            let ((top_z, upper), (bottom_z, lower)) = (at(step), at(step + 1.0));
            PI * (bottom_z - top_z) / 3.0 * (upper * upper + upper * lower + lower * lower)
        })
        .sum();
    let flare = fillet + PI * flare_bottom_r * flare_bottom_r * foot_h * FOOT_BITE;
    let expected = tip + neck + pillar + seam + pad + flare;

    let volume = signed_volume(&mesh);
    assert!(
        (volume - expected).abs() / expected < 0.001,
        "expected {expected} mm3 from frustums, cylinders and a disc, got {volume}"
    );
}

#[test]
fn the_pillar_meets_its_pad_on_a_quarter_circle() {
    let profile = profile();
    let trees = column_on_plate(20.0, &profile);
    let style = Style::of(&trees[0], &profile);
    let tube = flare(&trees[0], &profile, style).expect("a column on the plate flares");

    let (top, bottom) = (tube.rings[0], tube.rings[FLARE_RINGS as usize]);
    let widens = bottom.radius_mm - top.radius_mm;
    let rises = bottom.along_mm - top.along_mm;
    assert!(
        (widens - rises).abs() < 1e-5,
        "a quarter circle rises as far as it widens: {rises} mm up against {widens} mm out"
    );
    for ring in &tube.rings[..=FLARE_RINGS as usize] {
        let off = Vec2::new(ring.along_mm, ring.radius_mm - bottom.radius_mm).length();
        assert!(
            (off - rises).abs() < 1e-5,
            "every ring lies on the circle round the fillet's corner, got {off}"
        );
    }
    assert!(
        (tube.top.z - (profile.bottom.platform_thickness_mm + rises)).abs() < 1e-5,
        "the flare stands on the top of the pad"
    );
}

#[test]
fn a_flare_never_rises_above_the_pillar_it_widens() {
    let profile = profile();
    // Barely taller than its own foot, so there is next to no pillar to widen in.
    let trees = column_on_plate(profile.bottom.platform_thickness_mm + 0.3, &profile);
    let style = Style::of(&trees[0], &profile);
    let root_z = trees[0].root().position.z;
    if let Some(tube) = flare(&trees[0], &profile, style) {
        assert!(
            tube.top.z <= root_z + 1e-5,
            "a flare taller than the pillar would stand out of the top of it: \
                 {} against {root_z}",
            tube.top.z
        );
    }
    assert_eq!(
        diagnose(&meshed(&trees, Profiles::single(&profile))).boundary_edges,
        0
    );
}

#[test]
fn a_straight_pad_is_one_trapezoid_narrowing_to_the_plate() {
    let profile = profile();
    let trees = column_on_plate(20.0, &profile);
    let style = Style::of(&trees[0], &profile);
    let tube = foot(&trees[0], &profile, style).expect("a column on the plate has a pad");

    assert_eq!(
        tube.rings.len(),
        2,
        "a rim at the top and the plate at the bottom"
    );
    assert!(
        tube.rings[1].radius_mm < tube.rings[0].radius_mm,
        "the rim a blade gets under is the narrow one; see `docs/decisions/0044`"
    );
}

#[test]
fn every_column_adds_the_same_number_of_faces() {
    let profile = profile();
    let one = meshed(&column_on_plate(20.0, &profile), Profiles::single(&profile));

    let two = trees_on(
        &[
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(40.0, 10.0, 20.0)),
        ],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile),
    );
    assert_eq!(two.len(), 2, "thirty millimetres apart is too far to merge");
    let two = meshed(&two, Profiles::single(&profile));

    assert_eq!(two.faces.len(), one.faces.len() * 2);
    assert_eq!(two.vertices.len(), one.vertices.len() * 2);
    assert_eq!(diagnose(&two).boundary_edges, 0);
}

#[test]
fn a_branched_pair_is_closed_and_solid() {
    let profile = profile();
    let trees = trees_on(
        &[
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)),
        ],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile),
    );
    assert_eq!(trees.len(), 1);

    let mesh = meshed(&trees, Profiles::single(&profile));
    let diagnostics = diagnose(&mesh);
    assert_eq!(
        diagnostics.boundary_edges, 0,
        "every tube of a branch is closed on its own"
    );
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.degenerate_faces, 0);
    assert!(signed_volume(&mesh) > 0.0);
}

#[test]
fn a_neck_shortened_over_the_plate_stops_where_the_body_begins() {
    // A 2 mm top segment under a contact 1.5 mm up: the neck is cut to the room there
    // is, and what is drawn of it must be cut with it.
    let mut profile = profile();
    profile.branching.enabled = false;
    let points = [SupportPoint::new(Vec3::new(0.0, 0.0, 1.5))];
    let mesh = meshed(
        &trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        ),
        Profiles::single(&profile),
    );

    assert!(
        !mesh.is_empty(),
        "a contact 1.5 mm up has room for a support"
    );
    let lowest = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(Scalar::INFINITY, Scalar::min);
    assert!(
        lowest >= -1e-4,
        "a support ends on the plate, and this one reaches {lowest} mm"
    );
}

#[test]
fn branching_uses_less_material_than_two_columns() {
    let profile = profile();
    let straight = SupportProfile {
        branching: printer_profiles::Branching {
            enabled: false,
            ..profile.branching
        },
        ..profile.clone()
    };
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 40.0)),
        SupportPoint::new(Vec3::new(14.0, 10.0, 40.0)),
    ];

    let merged = meshed(
        &trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        ),
        Profiles::single(&profile),
    );
    let apart = meshed(
        &trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&straight),
        ),
        Profiles::single(&straight),
    );

    assert!(
        signed_volume(&merged) < signed_volume(&apart),
        "one trunk under two tips holds less resin than two full columns: {} against {}",
        signed_volume(&merged),
        signed_volume(&apart)
    );
}

#[test]
fn a_head_leaning_off_its_face_marks_it_with_a_circle_of_the_contact() {
    use core_geometry::Quat;
    use std::f32::consts::PI;

    // A steep ramp turned over: its face's normal is 68 degrees off straight down, so
    // the head leaves it at 60 and leans 8 degrees off the normal.
    let model = ramp(4.0, 10.0);
    let transform = Transform {
        rotation: Quat::from_rotation_x(PI),
        translation: Vec3::new(0.0, 0.0, 30.0),
        ..Transform::default()
    };
    let mut fixture = profile();
    fixture.tip.contact_depth_mm = 0.0;
    fixture.branching.enabled = false;
    let profiles = Profiles::single(&fixture);
    let local = Vec3::new(2.0, 0.0, 5.0);
    let contact = transform.to_matrix().transform_point3(local);
    let trees = trees_on(&[SupportPoint::new(local)], &model, transform, profiles);
    let bvh = core_geometry::Bvh::build(&model);
    let mesh = mesh_trees(&trees, &Placed::new(&model, &bvh, transform), profiles);

    let normal = Vec3::new(-10.0, 0.0, -4.0).normalize();
    let radius_mm = fixture.contact_radius_mm();
    let mut deepest: Scalar = 0.0;
    for vertex in mesh
        .vertices
        .iter()
        .filter(|v| (**v - contact).dot(normal) <= 1e-4)
    {
        deepest = deepest.min((*vertex - contact).dot(normal));
        assert!(
            (*vertex - contact).length() <= radius_mm + 1e-4,
            "what reaches the face is a ball of the contact's radius on it, got {vertex}"
        );
    }
    assert!(
        deepest < -0.9 * radius_mm,
        "half the ball is in the part at a bite of nought, got {deepest}"
    );
}

#[test]
fn a_tip_on_a_sloping_face_bites_into_it_along_the_normal() {
    use core_geometry::Quat;
    use std::f32::consts::PI;

    // A shallow ramp turned over, so the face that looked up is an overhang within 45
    // degrees of level, which a tip still leaves square on.
    let model = ramp(10.0, 4.0);
    let transform = Transform {
        rotation: Quat::from_rotation_x(PI),
        translation: Vec3::new(0.0, 0.0, 30.0),
        ..Transform::default()
    };
    let profile = profile();
    let contact = transform
        .to_matrix()
        .transform_point3(Vec3::new(5.0, 0.0, 2.0));
    let trees = trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 0.0, 2.0))],
        &model,
        transform,
        Profiles::single(&profile),
    );

    let normal = Vec3::new(-4.0, 0.0, -10.0).normalize();
    let mesh = meshed(&trees, Profiles::single(&profile));
    let apex = mesh
        .vertices
        .iter()
        .copied()
        .max_by(|a, b| {
            (*a - contact)
                .dot(-normal)
                .total_cmp(&(*b - contact).dot(-normal))
        })
        .expect("the support was meshed");
    let depth = profile.tip.contact_depth_mm;
    let radius_mm = profile.contact_radius_mm();
    assert!(
        ((apex - contact).dot(-normal) - depth - radius_mm).abs() < 1e-3,
        "the half ball is centred its bite into the face, got its pole at {}",
        (apex - contact).dot(-normal)
    );
    let centre = contact - normal * depth;
    for vertex in mesh
        .vertices
        .iter()
        .filter(|v| (**v - contact).dot(-normal) > depth)
    {
        assert!(
            (*vertex - centre).length() <= radius_mm + 1e-3,
            "past its centre the tip is a ball of the contact's radius, got {vertex}"
        );
    }
}

#[test]
fn nothing_of_a_plain_column_is_wider_than_its_pillar() {
    let profile = profile();
    let trees = column_on_plate(20.0, &profile);
    let mesh = meshed(&trees, Profiles::single(&profile));
    let widest = mesh
        .vertices
        .iter()
        .filter(|vertex| vertex.z > profile.bottom.platform_thickness_mm + 1.0)
        .map(|vertex| Vec2::new(vertex.x - 10.0, vertex.y - 10.0).length())
        .fold(0.0, Scalar::max);
    assert!(
        widest <= profile.pillar_radius_mm() + 1e-4,
        "nothing above the foot is wider than the pillar: {widest} mm"
    );
}

#[test]
fn a_strut_ends_rounded_inside_its_trunk_and_nothing_swells_round_the_joint() {
    let profile = profile();
    let trees = trees_on(
        &[
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)),
        ],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile),
    );
    let tree = &trees[0];
    let root = tree.root_index();
    let joint = tree.root().position;
    let trunk_mm = tree.root().radius_mm;
    let style = Style::of(tree, &profile);

    for (child, parent) in tree.struts().filter(|(_, parent)| *parent == root) {
        let tube = strut(tree, child, parent, &profile, style);
        let reaches = (joint - tree.nodes()[child].position).length();
        let own_mm = tree.nodes()[child].radius_mm;
        let widest = tube
            .rings
            .iter()
            .map(|ring| ring.radius_mm)
            .fold(0.0, Scalar::max);
        assert!(
            (widest - own_mm).abs() < 1e-5,
            "a strut keeps its own width of {own_mm} mm and does not widen into the \
                 trunk, got {widest}"
        );

        let mut swept = Mesh::default();
        sweep(&mut swept, &tube);
        let past: Vec<Scalar> = swept
            .vertices
            .iter()
            .filter(|vertex| (**vertex - tube.top).dot(tube.direction) > reaches + 1e-4)
            .map(|vertex| (*vertex - joint).length())
            .collect();
        assert!(!past.is_empty(), "the strut is rounded off past the joint");
        assert!(
            past.iter().all(|far| *far <= trunk_mm + 1e-4),
            "what is past the joint is inside the trunk's own dome of {trunk_mm} mm: \
                 {past:?}"
        );
    }

    let mesh = meshed(&trees, Profiles::single(&profile));
    let widest = mesh
        .vertices
        .iter()
        .filter(|vertex| vertex.z < joint.z && vertex.z > joint.z - 2.0 * trunk_mm)
        .map(|vertex| Vec2::new(vertex.x - joint.x, vertex.y - joint.y).length())
        .fold(0.0, Scalar::max);
    assert!(
        widest <= trunk_mm + 1e-4,
        "under the joint there is the trunk and nothing swelling round it: {widest} mm \
             against a trunk of {trunk_mm}"
    );
}

#[test]
fn no_trees_make_no_mesh() {
    assert!(meshed(&[], Profiles::single(&profile())).is_empty());
}

/// A column of `profile`, meshed.
fn one_column(profile: &SupportProfile) -> Mesh {
    meshed(&column_on_plate(20.0, profile), Profiles::single(profile))
}

/// The highest point the mesh reaches, which is how far the tip bit into the model.
fn ceiling(mesh: &Mesh) -> Scalar {
    mesh.vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(Scalar::NEG_INFINITY, Scalar::max)
}

/// Vertices sitting on the plate, which is the foot's own ring plus the trunk's.
fn on_the_plate(mesh: &Mesh) -> usize {
    mesh.vertices
        .iter()
        .filter(|vertex| vertex.z.abs() < 1e-4)
        .count()
}

fn with_tip(shape: ContactShape) -> SupportProfile {
    let mut profile = profile();
    profile.tip.shape = shape;
    profile
}

fn with_foot(shape: printer_profiles::PlatformShape) -> SupportProfile {
    let mut profile = profile();
    profile.bottom.shape = shape;
    profile
}

#[test]
fn every_tip_shape_meshes_into_a_closed_solid() {
    for shape in [ContactShape::Sphere, ContactShape::Plane] {
        let profile = with_tip(shape);
        let diagnostics = diagnose(&one_column(&profile));
        assert_eq!(
            diagnostics.boundary_edges, 0,
            "a {shape:?} tip left the column open"
        );
        assert_eq!(diagnostics.non_manifold_edges, 0, "{shape:?}");
        assert_eq!(diagnostics.degenerate_faces, 0, "{shape:?}");
    }
}

#[test]
fn a_plane_tip_ends_in_a_half_ball_of_its_contact_pushed_its_bite_in() {
    let flat = with_tip(ContactShape::Plane);
    let mesh = one_column(&flat);
    let centre = 20.0 + flat.tip.contact_depth_mm;
    assert!(
        (ceiling(&mesh) - (centre + flat.contact_radius_mm())).abs() < 1e-4,
        "the half ball's pole is a contact radius past its bite into the face at z = 20, \
             got {}",
        ceiling(&mesh)
    );
    let axis = Vec3::new(10.0, 10.0, 0.0);
    assert!(
        (widest_above(&mesh, axis, centre - 2e-3) - flat.contact_radius_mm()).abs() < 1e-3,
        "it is as wide as its contact where it is centred, and no wider"
    );
}

#[test]
fn a_plane_tip_biting_past_its_top_segment_keeps_its_pole_on_the_half_ball() {
    let mut deep = with_tip(ContactShape::Plane);
    deep.tip.contact_depth_mm = deep.top.length_mm + 1.0;
    let mesh = one_column(&deep);
    let pole = 20.0 + deep.top.length_mm + deep.contact_radius_mm();
    assert!(
        (ceiling(&mesh) - pole).abs() < 1e-4,
        "the ball is centred no deeper than the top segment, and its pole a contact \
             radius past that: expected {pole}, got {}",
        ceiling(&mesh)
    );
}

/// How wide the mesh is above `z`, measured from the support's own axis.
fn widest_above(mesh: &Mesh, axis: Vec3, z: Scalar) -> Scalar {
    mesh.vertices
        .iter()
        .filter(|vertex| vertex.z > z + 1e-3)
        .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
        .fold(0.0, Scalar::max)
}

#[test]
fn a_sphere_tip_is_wide_inside_the_model_and_bites_its_depth() {
    let ball = with_tip(ContactShape::Sphere);
    let axis = Vec3::new(10.0, 10.0, 0.0);

    assert!(
        (ceiling(&one_column(&ball)) - (20.0 + ball.tip.contact_depth_mm)).abs() < 1e-4,
        "the top of the ball is its bite into the model"
    );
    assert!(
        widest_above(&one_column(&ball), axis, 20.0) > ball.contact_radius_mm() * 0.8,
        "a ball is still most of its own width inside the model, which is the grip \
             it is chosen for"
    );
}

#[test]
fn a_cube_foot_stands_on_four_corners_and_a_cylinder_on_every_facet() {
    use printer_profiles::PlatformShape;
    let cube = with_foot(PlatformShape::Cube);
    let round = with_foot(PlatformShape::Cylinder);

    let difference = on_the_plate(&one_column(&round)) - on_the_plate(&one_column(&cube));
    assert_eq!(
        difference,
        round.facets as usize - 4,
        "a cube foot has four corners where a {}-sided one has {}",
        round.facets,
        round.facets
    );
}

#[test]
fn every_foot_shape_meshes_into_a_closed_solid() {
    use printer_profiles::PlatformShape;
    for shape in [
        PlatformShape::Cylinder,
        PlatformShape::Cone,
        PlatformShape::Prism,
        PlatformShape::Cube,
    ] {
        let diagnostics = diagnose(&one_column(&with_foot(shape)));
        assert_eq!(
            diagnostics.boundary_edges, 0,
            "a {shape:?} foot left the support open"
        );
        assert_eq!(diagnostics.degenerate_faces, 0, "{shape:?}");
    }
}

#[test]
fn a_tapering_foot_holds_less_than_a_straight_one_of_the_same_rim() {
    use printer_profiles::PlatformShape;
    let tapered = one_column(&with_foot(PlatformShape::Cone));
    let straight = one_column(&with_foot(PlatformShape::Cylinder));
    assert!(
        signed_volume(&tapered) < signed_volume(&straight),
        "a foot that narrows towards the support is less resin than a full pad"
    );
}

/// A support standing on the model rather than on the plate: a shelf 10 mm up with a
/// contact 10 mm above that.
fn model_to_model(profile: &SupportProfile) -> Vec<SupportTree> {
    let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))],
        &shelf,
        Transform::default(),
        Profiles::single(profile),
    )
}

#[test]
fn a_support_between_two_surfaces_is_thinned_to_the_small_pillar() {
    let profile = profile();
    let mut regular = profile.clone();
    regular.small_pillar.enabled = false;

    let trees = model_to_model(&profile);
    assert!(trees[0].landing().on_model, "it stands on the shelf");

    let thin = signed_volume(&meshed(&trees, Profiles::single(&profile)));
    let thick = signed_volume(&meshed(
        &model_to_model(&regular),
        Profiles::single(&regular),
    ));
    assert!(
        thin < thick,
        "a model-to-model strut uses the small pillar's own width: {thin} mm3 \
             against {thick} mm3"
    );
    assert_eq!(
        diagnose(&meshed(&trees, Profiles::single(&profile))).boundary_edges,
        0
    );
}

/// The highest point of the support held under a 40 mm slab from `bottom_mm` up, whose
/// contact is in the middle of its underside.
fn tip_top_under_slab(bottom_mm: Scalar, thickness_mm: Scalar) -> Scalar {
    let slab = crate::fixtures::box_mesh(
        Vec3::new(-20.0, -20.0, bottom_mm),
        Vec3::new(20.0, 20.0, bottom_mm + thickness_mm),
    );
    let mut fixture = profile();
    fixture.branching.enabled = false;
    let points = [crate::SupportPoint::new(Vec3::new(0.0, 0.0, bottom_mm))];
    let trees = crate::fixtures::trees_on(
        &points,
        &slab,
        Transform::default(),
        Profiles::single(&fixture),
    );
    let bvh = core_geometry::Bvh::build(&slab);
    let mesh = mesh_trees(
        &trees,
        &Placed::new(&slab, &bvh, Transform::default()),
        Profiles::single(&fixture),
    );
    mesh.vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(Scalar::NEG_INFINITY, Scalar::max)
}

#[test]
fn a_tip_under_a_thin_wall_bites_no_further_than_half_of_it() {
    let top = tip_top_under_slab(10.0, 0.15);
    assert!(
        top < 10.15,
        "a 0.2 mm bite into a 0.15 mm wall would come out of its top, got {top}"
    );
    assert!(
        (top - 10.075).abs() < 0.01,
        "half the wall is bitten, got {top}"
    );
}

#[test]
fn a_strut_straight_from_its_contact_bites_no_further_than_half_the_wall() {
    // A 0.15 mm ledge a millimetre over a block: one strut from the contact itself.
    let mut model = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 9.0));
    let ledge = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 10.15));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&ledge.vertices);
    model
        .faces
        .extend(ledge.faces.iter().map(|face| face.map(|at| at + offset)));
    let fixture = profile();
    let points = [SupportPoint::new(Vec3::new(5.0, 5.0, 10.0))];
    let trees = trees_on(
        &points,
        &model,
        Transform::default(),
        Profiles::single(&fixture),
    );
    assert!(trees[0].root().is_leaf(), "one node, the contact");

    let bvh = core_geometry::Bvh::build(&model);
    let mesh = mesh_trees(
        &trees,
        &Placed::new(&model, &bvh, Transform::default()),
        Profiles::single(&fixture),
    );
    let top = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.z)
        .fold(Scalar::NEG_INFINITY, Scalar::max);
    assert!(
        (top - 10.075).abs() < 0.01,
        "half the 0.15 mm wall is bitten, got {top}"
    );
}

#[test]
fn a_tip_under_a_thick_wall_bites_its_whole_depth() {
    let top = tip_top_under_slab(10.0, 5.0);
    assert!(
        (top - 10.4).abs() < 0.01,
        "the fixture's 0.2 mm bite and the half ball's 0.2 mm radius, got {top}"
    );
}

#[test]
fn a_support_between_two_surfaces_longer_than_the_small_pillar_keeps_its_width() {
    let mut short = profile();
    short.small_pillar.max_length_mm = 0.1;
    let mut regular = profile();
    regular.small_pillar.enabled = false;

    let limited = signed_volume(&meshed(&model_to_model(&short), Profiles::single(&short)));
    let thick = signed_volume(&meshed(
        &model_to_model(&regular),
        Profiles::single(&regular),
    ));
    assert!(
        (limited - thick).abs() < 1e-3,
        "a run past 0.1 mm is built as a regular support: {limited} mm3 against {thick} mm3"
    );
}

#[test]
fn a_small_pillar_meets_sloping_faces_with_the_whole_of_each_end() {
    // Underside z = 12 - x / 2 over top z = 11 - x / 2: faces at 26.6 degrees a
    // millimetre apart, too near for a head, so the strut runs straight down.
    let mut model = Mesh::default();
    let ramp = ramp(10.0, 5.0);
    for place in [
        (|v: Vec3| Vec3::new(v.x, v.y, 12.0 - v.z)) as fn(Vec3) -> Vec3,
        |v: Vec3| Vec3::new(10.0 - v.x, v.y, v.z + 6.0),
    ] {
        let offset = model.vertices.len() as u32;
        model
            .vertices
            .extend(ramp.vertices.iter().map(|v| place(*v)));
        // Both turn the ramp inside out, so its faces are wound back.
        model.faces.extend(
            ramp.faces
                .iter()
                .map(|face| [face[0] + offset, face[2] + offset, face[1] + offset]),
        );
    }
    let fixture = profile();
    let points = [SupportPoint::new(Vec3::new(5.0, 0.0, 9.5))];
    let trees = trees_on(
        &points,
        &model,
        Transform::default(),
        Profiles::single(&fixture),
    );
    assert!(
        trees[0].root().is_leaf(),
        "one strut, from the contact itself"
    );
    assert!(Style::of(&trees[0], &fixture).small, "it runs face to face");

    let bvh = core_geometry::Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mesh = mesh_trees(&trees, &placed, Profiles::single(&fixture));
    // Each end is laid on its face and then sunk along the axis, so every point of
    // it stands the same height off that face.
    for (name, upper) in [("top", true), ("bottom", false)] {
        let heights: Vec<Scalar> = mesh
            .vertices
            .iter()
            .filter(|vertex| (vertex.z > 9.0) == upper)
            .map(|v| {
                if upper {
                    12.0 - v.x / 2.0 - v.z
                } else {
                    v.z - 11.0 + v.x / 2.0
                }
            })
            .collect();
        let low = heights.iter().copied().fold(Scalar::INFINITY, Scalar::min);
        let high = heights
            .iter()
            .copied()
            .fold(Scalar::NEG_INFINITY, Scalar::max);
        assert!(
            !heights.is_empty() && high - low < 1e-3,
            "the {name} end lies along its face, got {low}..{high} off it"
        );
    }
    assert_eq!(diagnose(&mesh).boundary_edges, 0, "it is closed");
}

#[test]
fn a_small_pillar_has_no_tip_and_is_one_width_from_end_to_end() {
    for shape in [ContactShape::Plane, ContactShape::Sphere] {
        let mut profile = profile();
        profile.tip.shape = shape;
        let trees = model_to_model(&profile);
        let mesh = meshed(&trees, Profiles::single(&profile));
        let radius_mm = profile.small_pillar.radius_mm();

        // Within a millimetre of either face: the joint under the neck is a dome
        // inside the body, and is not what touches the model.
        let ends = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.z > 19.0 || vertex.z < 11.0);
        for vertex in ends {
            let off_axis = Vec2::new(vertex.x - 5.0, vertex.y - 5.0).length();
            // The middle of each flat end sits on the axis; everything else is wall.
            assert!(
                off_axis < 1e-3 || (off_axis - radius_mm).abs() < 1e-3,
                "{shape:?}: a vertex {off_axis} mm off the axis of a pillar \
                     {radius_mm} mm in radius is a tip or a taper"
            );
        }
        assert_eq!(diagnose(&mesh).boundary_edges, 0, "{shape:?}: it is closed");
    }
}

/// How far from `axis` the mesh reaches at or below `z`, which under a support
/// standing on the model is the plug driven into the surface.
fn widest_below(mesh: &Mesh, axis: Vec3, z: Scalar) -> Scalar {
    mesh.vertices
        .iter()
        .filter(|vertex| vertex.z < z + 1e-3)
        .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
        .fold(0.0, Scalar::max)
}

#[test]
fn a_support_standing_on_the_model_is_pushed_its_bite_into_it() {
    let mut profile = profile();
    profile.small_pillar.enabled = false;
    let trees = model_to_model(&profile);
    assert!(trees[0].landing().on_model, "it stands on the shelf");

    let mesh = meshed(&trees, Profiles::single(&profile));
    let lowest = mesh
        .vertices
        .iter()
        .copied()
        .min_by(|a, b| a.z.total_cmp(&b.z))
        .expect("a support has vertices");
    let sunk = 10.0 - profile.landing_depth_mm();
    assert!(
        (lowest.z - (sunk - profile.contact_radius_mm())).abs() < 1e-3,
        "the half ball is centred the bite into the 10 mm shelf, got {}",
        lowest.z
    );
    let widest = widest_below(&mesh, Vec3::new(5.0, 5.0, 0.0), sunk);
    assert!(
        widest <= profile.contact_radius_mm() + 1e-3,
        "it ends in a half ball of its contact, as the plane tip at the top does, got {widest}"
    );
    assert_eq!(diagnose(&mesh).boundary_edges, 0);
}

#[test]
fn a_support_standing_on_the_model_touches_it_with_a_contact_not_a_trunk() {
    let mut profile = profile();
    profile.small_pillar.enabled = false;
    let trees = model_to_model(&profile);
    assert!(trees[0].landing().on_model, "it stands on the shelf");

    let mesh = meshed(&trees, Profiles::single(&profile));
    let widest = widest_below(&mesh, Vec3::new(5.0, 5.0, 0.0), 10.0);
    assert!(
        widest <= profile.contact_radius_mm() + 1e-3,
        "what enters the shelf is the tip's own contact: {widest} mm against              {} mm",
        profile.contact_radius_mm()
    );
    assert!(
        widest < profile.pillar_radius_mm(),
        "the trunk is not cured into the part at its full width"
    );
    assert_eq!(diagnose(&mesh).boundary_edges, 0);
}

/// Three tips over the same shelf, close enough to merge into one trunk wider than
/// the top segment it ends in.
fn merged_on_the_model(profile: &SupportProfile) -> Vec<SupportTree> {
    // Further over the shelf than a small pillar reaches, which would stand each alone.
    let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let points = [
        SupportPoint::new(Vec3::new(4.0, 5.0, 25.0)),
        SupportPoint::new(Vec3::new(6.0, 5.0, 25.0)),
        SupportPoint::new(Vec3::new(5.0, 6.5, 25.0)),
    ];
    trees_on(
        &points,
        &shelf,
        Transform::default(),
        Profiles::single(profile),
    )
}

#[test]
fn a_trunk_on_the_model_keeps_its_width_down_to_its_tip() {
    let profile = profile();
    let trees = merged_on_the_model(&profile);
    let [tree] = &trees[..] else {
        panic!("three tips this close merge into one trunk");
    };
    assert!(tree.landing().on_model, "it stands on the shelf");
    let trunk_mm = trunk_base_mm(tree, &profile, Style::of(tree, &profile));
    assert!(
        (trunk_mm - profile.pillar_radius_mm()).abs() < 1e-6,
        "a trunk on the model keeps the branches' width down to its tip, however many \
             it carries, got {trunk_mm}"
    );

    let mesh = meshed(&trees, Profiles::single(&profile));
    let axis = tree.landing().base;
    let widths: Vec<Scalar> = mesh
        .vertices
        .iter()
        .filter(|vertex| {
            let shoulder = 10.0 - profile.landing_depth_mm() + profile.top.length_mm;
            (vertex.z - shoulder).abs() < 1e-3
        })
        .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
        .collect();
    assert!(!widths.is_empty(), "the taper starts one top segment up");
    for width in widths {
        assert!(
            (width - trunk_mm).abs() < 1e-3,
            "the cone starts at the trunk's width, not at a rim under it: {width} mm                  against {trunk_mm} mm"
        );
    }
}

#[test]
fn a_trunk_widens_from_its_joint_down_to_its_base() {
    let profile = profile();
    let trees = trees_on(
        &[
            SupportPoint::new(Vec3::new(10.0, 10.0, 40.0)),
            SupportPoint::new(Vec3::new(14.0, 10.0, 40.0)),
        ],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile),
    );
    let [tree] = &trees[..] else {
        panic!("two tips this close merge into one trunk");
    };
    let tube = trunk(tree, &profile, Style::of(tree, &profile));
    let pillar = profile.pillar_radius_mm();
    let last = tube.rings[tube.rings.len() - 1].radius_mm;
    let height = (tree.root().position - trunk_bottom(tree, &profile)).length();
    let base = profile.trunk_base_radius_mm(height, 2);
    assert!(
        tube.rings
            .iter()
            .any(|ring| (ring.radius_mm - pillar).abs() < 1e-5),
        "the trunk leaves its joint at the branches' own width of {pillar} mm"
    );
    assert!(
        (last - base).abs() < 1e-5 && last > pillar,
        "it widens to the {base} mm its height and two tips ask for, got {last}"
    );
}

#[test]
fn a_profile_that_refuses_the_model_leaves_no_support_standing_on_it() {
    let mut profile = profile();
    profile.land_on_model = false;
    let trees = model_to_model(&profile);
    assert!(
        trees.iter().all(|tree| !tree.landing().on_model),
        "a support over the shelf leans for the plate or is dropped"
    );
}

#[test]
fn a_plate_with_skates_a_raft_and_bracing_is_still_closed() {
    use printer_profiles::{Bracing, PlatformShape, Raft};
    let mut profile = profile();
    profile.bottom.shape = PlatformShape::Skate;
    profile.branching.enabled = false;
    profile.raft = Raft {
        enabled: true,
        ..Raft::default()
    };
    profile.bracing = Bracing {
        enabled: true,
        start_height_mm: 5.0,
        rise_mm: 5.0,
        ..Bracing::default()
    };

    let points: Vec<SupportPoint> = [(10.0, 10.0), (18.0, 10.0), (14.0, 18.0)]
        .into_iter()
        .map(|(x, y)| SupportPoint::new(Vec3::new(x, y, 30.0)))
        .collect();
    let trees = trees_on(
        &points,
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile),
    );
    assert_eq!(trees.len(), 3);

    let mesh = meshed(&trees, Profiles::single(&profile));
    let diagnostics = diagnose(&mesh);
    assert_eq!(
        diagnostics.boundary_edges, 0,
        "every piece closes on its own"
    );
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.degenerate_faces, 0);
    assert!(signed_volume(&mesh) > 0.0);

    let bounds = mesh.aabb().expect("the plate has geometry");
    assert!(
        bounds.mins.z.abs() < 1e-4,
        "the raft sits on the plate, got {}",
        bounds.mins.z
    );
}

#[test]
fn nothing_of_a_support_shows_through_the_bottom_of_its_foot() {
    let profile = profile();
    let mesh = one_column(&profile);
    let rim = profile.base_radius_mm() - profile.bottom.platform_thickness_mm * FOOT_BEVEL;
    let axis = Vec3::new(10.0, 10.0, 0.0);

    for vertex in &mesh.vertices {
        assert!(
            vertex.z > -1e-4,
            "nothing reaches under the plate, got {vertex}"
        );
        if vertex.z < 1e-4 {
            let reach = Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length();
            assert!(
                reach <= rim + 1e-3 || reach < 1e-4,
                "only the foot's own rim touches the plate, got {reach} mm out"
            );
        }
    }

    let trunk_on_the_plate = mesh
        .vertices
        .iter()
        .filter(|vertex| vertex.z < 1e-4)
        .filter(|vertex| {
            let reach = Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length();
            (reach - profile.pillar_radius_mm()).abs() < 1e-3
        })
        .count();
    assert_eq!(
        trunk_on_the_plate, 0,
        "the trunk stops inside the foot, it does not show through its sole"
    );
}

#[test]
fn the_cone_under_a_tip_runs_from_the_contact_out_to_the_pillar() {
    let profile = profile();
    let mesh = one_column(&profile);
    let axis = Vec3::new(10.0, 10.0, 0.0);
    let width_at = |z: Scalar| {
        mesh.vertices
            .iter()
            .filter(|vertex| (vertex.z - z).abs() < 1e-3)
            .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
            .fold(0.0, Scalar::max)
    };

    let sunk = 20.0 + profile.tip.contact_depth_mm;
    assert!(
        (width_at(sunk) - profile.contact_radius_mm()).abs() < 1e-3,
        "the cone starts its bite inside the model as wide as the contact, got {}",
        width_at(sunk)
    );
    let shoulder = sunk - profile.top.length_mm;
    assert!(
        (width_at(shoulder) - profile.pillar_radius_mm()).abs() < 1e-3,
        "and meets the pillar at the pillar's own width, got {}",
        width_at(shoulder)
    );
}

#[test]
fn a_support_on_a_raft_has_no_pad_and_flares_onto_the_raft() {
    let mut profile = profile();
    profile.raft.enabled = true;
    let trees = column_on_plate(20.0, &profile);
    let model = Mesh::default();
    let bvh = core_geometry::Bvh::build(&model);
    let mesh = mesh_alone(
        &trees[0],
        Profiles::single(&profile),
        &Placed::new(&model, &bvh, Transform::default()),
    );
    let thickness = profile.raft.thickness_mm;
    let flare_mm = profile.flare_lower_radius_mm();
    assert!(
        flare_mm < profile.base_radius_mm(),
        "the fixture's flare is narrower than its pad, or this proves nothing"
    );

    let axis = Vec2::new(10.0, 10.0);
    for vertex in &mesh.vertices {
        assert!(
            vertex.z >= thickness * (1.0 - FOOT_BITE) - 1e-4,
            "nothing reaches under the raft's top by more than the bite, got {vertex}"
        );
        let reach = (vertex.truncate() - axis).length();
        assert!(
            reach <= flare_mm + 1e-3,
            "nothing is wider than the flare, got {reach} mm out at {vertex}"
        );
    }
    let style = Style::of(&trees[0], &profile);
    let tube = flare(&trees[0], &profile, style).expect("a column on a raft flares");
    let foot = tube.rings[FLARE_RINGS as usize];
    let rises = foot.along_mm - tube.rings[0].along_mm;
    assert!(
        (foot.radius_mm - flare_mm).abs() < 1e-5,
        "the flare widens to its own lower diameter, not to what a pad would allow"
    );
    assert!(
        (tube.top.z - (thickness + rises)).abs() < 1e-5,
        "the flare stands on the top of the raft"
    );
}

#[test]
fn a_support_that_reaches_the_plate_is_never_thinned() {
    let profile = profile();
    let mut regular = profile.clone();
    regular.small_pillar.enabled = false;

    let thin = signed_volume(&one_column(&profile));
    let thick = signed_volume(&one_column(&regular));
    assert!(
        (thin - thick).abs() < 1e-3,
        "the small pillar is for struts between surfaces, not for columns"
    );
}
