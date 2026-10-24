use super::held::Held;
use crate::config::{GRID_PITCH_MM, SampleConfig};
use crate::fixtures::box_mesh;
use crate::need::{Need, Reason};
use crate::region::Blocked;
use core_geometry::Mesh;
use core_geometry::{Scalar, Vec2, Vec3};
use core_slicer::Sliced;
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use printer_profiles::SupportProfile;

/// Layer height every test in this module cuts at, millimetres.
const LAYER_HEIGHT_MM: Scalar = 0.05;

fn profile() -> SupportProfile {
    crate::fixtures::profile()
}

fn slice(mesh: &Mesh) -> Sliced {
    PlaneSliceEngine
        .slice(
            mesh,
            &SliceSettings {
                layer_height: LAYER_HEIGHT_MM,
                ..SliceSettings::default()
            },
        )
        .expect("a closed mesh slices")
}

/// The contacts of [`super::generate_supports`], which is what most of these tests ask
/// about.
fn generate_supports(
    stack: &Sliced,
    layer_height_mm: Scalar,
    profile: &SupportProfile,
    seeds: &[Vec3],
    lows: &[Vec3],
    blocked: Option<&Blocked>,
    progress: &mut dyn FnMut(usize) -> bool,
) -> Vec<Vec3> {
    super::generate_supports(
        stack,
        layer_height_mm,
        profile,
        None,
        seeds,
        lows,
        blocked,
        progress,
    )
    .into_iter()
    .map(|need| need.contact)
    .collect()
}

/// Everything automatic placement puts under `mesh`, with nothing there already.
fn supports_for(mesh: &Mesh) -> Vec<Vec3> {
    generate_supports(
        &slice(mesh),
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| true,
    )
}

fn merged(a: &Mesh, b: &Mesh) -> Mesh {
    let offset = a.vertices.len() as u32;
    let mut mesh = a.clone();
    mesh.vertices.extend_from_slice(&b.vertices);
    mesh.faces.extend(
        b.faces
            .iter()
            .map(|[x, y, z]| [x + offset, y + offset, z + offset]),
    );
    mesh
}

/// A ball of `radius` with its south pole at `z`, fine enough that its lower cap is a
/// smooth overhang rather than a staircase.
fn sphere(radius: Scalar, base_mm: Scalar) -> Mesh {
    let (segments, rings) = (96u32, 48u32);
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        for segment in 0..segments {
            let theta = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            vertices.push(Vec3::new(
                radius * phi.sin() * theta.cos(),
                radius * phi.sin() * theta.sin(),
                base_mm + radius - radius * phi.cos(),
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

/// A cone standing on its point `apex_mm` over the plate, opening to `radius` over
/// `height`: nothing on it leans further than its own wall.
fn cone_on_its_point(apex_mm: Scalar, radius: Scalar, height: Scalar) -> Mesh {
    let sides = 64u32;
    let mut vertices = vec![
        Vec3::new(0.0, 0.0, apex_mm),
        Vec3::new(0.0, 0.0, apex_mm + height),
    ];
    let mut faces = Vec::new();
    for side in 0..sides {
        let angle = std::f32::consts::TAU * side as Scalar / sides as Scalar;
        vertices.push(Vec3::new(
            radius * angle.cos(),
            radius * angle.sin(),
            apex_mm + height,
        ));
        let (here, next) = (2 + side, 2 + (side + 1) % sides);
        faces.push([0, next, here]);
        faces.push([1, here, next]);
    }
    Mesh::new(vertices, faces)
}

/// What placement wants under `mesh`, with what each contact holds.
fn needs_for(mesh: &Mesh) -> Vec<Need> {
    super::generate_supports(
        &slice(mesh),
        LAYER_HEIGHT_MM,
        &profile(),
        None,
        &[],
        &[],
        None,
        &mut |_| true,
    )
}

/// A 10 × 10 × 1 mm shelf on a 2 mm square post 5 mm tall, with a 10 × 10 mm tower
/// `tower_mm` tall on top of it.
fn shelf(tower_mm: Scalar) -> Mesh {
    let post = crate::fixtures::box_mesh(Vec3::new(9.0, 9.0, 0.0), Vec3::new(11.0, 11.0, 5.0));
    let slab = crate::fixtures::box_mesh(Vec3::new(5.0, 5.0, 5.0), Vec3::new(15.0, 15.0, 6.0));
    let mesh = merged(&post, &slab);
    if tower_mm <= 0.0 {
        return mesh;
    }
    let tower = Vec3::new(15.0, 15.0, 6.0 + tower_mm);
    merged(
        &mesh,
        &crate::fixtures::box_mesh(Vec3::new(5.0, 5.0, 6.0), tower),
    )
}

/// The model above every overhang contact under `mesh`, mm³.
fn carried(mesh: &Mesh) -> Vec<Scalar> {
    needs_for(mesh)
        .into_iter()
        .filter_map(|need| match need.reason {
            Reason::Overhang { carried_mm3 } => Some(carried_mm3),
            Reason::Island { .. } | Reason::Step { .. } | Reason::Steady => None,
        })
        .collect()
}

#[test]
fn a_floating_slab_is_held_as_an_island_of_its_own_area() {
    let slab = crate::fixtures::box_mesh(Vec3::new(2.0, 2.0, 5.0), Vec3::new(6.0, 5.0, 6.0));
    let needs = needs_for(&slab);
    let islands: Vec<Scalar> = needs
        .iter()
        .filter_map(|need| match need.reason {
            Reason::Island { area_mm2 } => Some(area_mm2),
            Reason::Overhang { .. } | Reason::Step { .. } | Reason::Steady => None,
        })
        .collect();
    assert!(!islands.is_empty());
    assert!(
        islands.iter().all(|mm2| (mm2 - 12.0).abs() < 0.5),
        "the slab is 4 × 3 mm, so its island is 12 mm², got {islands:?}"
    );
    assert!(
        !needs
            .iter()
            .any(|need| matches!(need.reason, Reason::Overhang { .. })),
        "nothing on a slab with nothing beneath it is an overhang"
    );
}

#[test]
fn an_overhang_carries_the_model_above_it() {
    let alone = carried(&shelf(0.0));
    let under_tower = carried(&shelf(20.0));
    assert!(!alone.is_empty() && !under_tower.is_empty());
    // The slab is 100 mm³ less its post and the layer the contacts are cut under; the
    // tower adds 2000 mm³ more.
    assert!(
        alone.iter().all(|mm3| (90.0..=100.0).contains(mm3)),
        "a bare shelf carries itself, got {alone:?}"
    );
    assert!(
        under_tower
            .iter()
            .all(|mm3| (2090.0..=2100.0).contains(mm3)),
        "a shelf under a 20 mm tower carries the tower too, got {under_tower:?}"
    );
}

#[test]
fn a_point_hanging_over_the_plate_is_held_once_it_is_wide_enough_to_print() {
    // A wall 27 degrees from vertical, well inside the 45 the profile allows, so
    // nothing on the cone is an overhang and its first layer is a speck.
    let apex_mm = 5.0;
    let points = supports_for(&cone_on_its_point(apex_mm, 5.0, 10.0));

    let lowest = points
        .iter()
        .map(|point| point.z)
        .fold(Scalar::INFINITY, Scalar::min);
    // The cone is `MIN_ISLAND_MM` across where its radius is half of that, and its
    // radius is half its height above the point: 0.4 mm up.
    assert!(
        lowest < apex_mm + 1.0,
        "a part with nothing under it hangs in mid-air; its lowest contact is at \
             {lowest} mm and its point at {apex_mm} mm"
    );
}

#[test]
fn a_small_ball_stands_on_more_than_the_support_under_its_pole() {
    let points = supports_for(&sphere(5.0, 5.0));
    assert!(
        points.len() >= 4,
        "a part on fewer than three supports rocks on the peel: one under the pole and \
             three round it, got {}",
        points.len()
    );
}

#[test]
fn every_corner_of_a_floating_slab_is_held() {
    // Off the lattice on every side, so no corner has a lattice point to lean on.
    let (low, high) = (Vec2::new(1.3, 2.7), Vec2::new(18.3, 15.7));
    let points = supports_for(&box_mesh(low.extend(10.0), high.extend(14.0)));
    let inset_mm = SampleConfig::for_profile(&profile()).edge_inset_mm;

    for corner in [
        low,
        Vec2::new(high.x, low.y),
        high,
        Vec2::new(low.x, high.y),
    ] {
        let nearest = points
            .iter()
            .map(|point| (point.truncate() - corner).length())
            .fold(Scalar::INFINITY, Scalar::min);
        assert!(
            nearest <= inset_mm * Scalar::sqrt(2.0) + 2.0 * GRID_PITCH_MM,
            "the corner at {corner} is {nearest} mm from the nearest support, and a \
                 corner is held one inset in from both of its sides"
        );
    }
}

#[test]
fn no_head_under_a_floating_slab_reaches_over_its_edge() {
    let (low, high) = (Vec2::new(1.3, 2.7), Vec2::new(18.3, 15.7));
    let points = supports_for(&box_mesh(low.extend(10.0), high.extend(14.0)));
    let radius_mm = profile().contact_radius_mm();

    assert!(!points.is_empty(), "a floating slab is an island");
    for point in &points {
        let at = point.truncate();
        let room_mm = (at - low).min_element().min((high - at).min_element());
        assert!(
            room_mm >= radius_mm,
            "the head at {at} is {room_mm} mm from the slab's edge and {radius_mm} mm \
                 wide, so it climbs the wall above that edge"
        );
    }
}

#[test]
fn a_part_that_begins_at_a_point_is_held_at_the_point_itself() {
    let apex = Vec3::new(0.0, 0.0, 5.0);
    let cone = cone_on_its_point(apex.z, 5.0, 10.0);
    let lows = crate::low_points(&cone, core_geometry::Transform::default());
    let points = generate_supports(
        &slice(&cone),
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &lows,
        None,
        &mut |_| true,
    );

    assert!(
        points.contains(&apex),
        "the cone hangs from its point at {apex}, and its lowest contact is {:?}",
        points.iter().min_by(|a, b| a.z.total_cmp(&b.z))
    );
}

#[test]
fn a_low_point_on_a_part_that_is_already_held_gets_no_support_of_its_own() {
    // A box on the plate, told of a point on its wall: nothing begins there.
    let standing = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));
    let points = generate_supports(
        &slice(&standing),
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[Vec3::new(0.0, 5.0, 5.0)],
        None,
        &mut |_| true,
    );
    assert!(points.is_empty(), "got {points:?}");
}

/// Two boxes, the upper one jutting `step` further out over the lower one's ledge:
/// a staircase of overhangs, one above the other, which is what a stepped model is.
fn steps(step: Scalar, rise: Scalar) -> Mesh {
    let lower = box_mesh(
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(10.0, 10.0, 10.0 + rise),
    );
    let upper = box_mesh(
        Vec3::new(0.0, -step, 10.0 + rise),
        Vec3::new(10.0, 10.0, 10.0 + rise + rise),
    );
    merged(&lower, &upper)
}

/// What a run sized by the shipped automatic profile wants under `mesh`.
fn sized_needs_for(mesh: &Mesh) -> Vec<Need> {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue");
    let auto = &catalogue
        .autos()
        .next()
        .expect("an automatic profile ships")
        .profile;
    super::generate_supports(
        &slice(mesh),
        LAYER_HEIGHT_MM,
        // Placed by the automatic profile's numbers, as the window places a sized run.
        &auto.placing_with(&profile()),
        Some(auto),
        &[],
        &[],
        None,
        &mut |_| true,
    )
}

/// A 20 mm square block on the plate, an optional 20 by 12 mm shelf at z = 5, and on that a 4 mm
/// block whose face over the shelf leans 30 degrees: past a step's 20, short of the fixture's 45.
fn shelf_under_a_lean(shelf: bool) -> Mesh {
    let base_y = if shelf { 0.0 } else { -12.0 };
    let base = box_mesh(Vec3::new(0.0, base_y, 0.0), Vec3::new(20.0, 20.0, 5.0));
    let slab = box_mesh(Vec3::new(0.0, -12.0, 5.0), Vec3::new(20.0, 20.0, 6.0));
    let mut lean = box_mesh(Vec3::new(0.0, -12.0, 6.0), Vec3::new(20.0, 20.0, 10.0));
    let out = 4.0 * 30.0_f32.to_radians().tan();
    // The two corners of the top face over the shelf's edge.
    for corner in [4, 5] {
        lean.vertices[corner].y -= out;
    }
    merged(&merged(&base, &slab), &lean)
}

/// The contacts under the leaning face, above the shelf's edge, with why each is there.
fn under_the_lean(needs: &[Need]) -> Vec<Reason> {
    needs
        .iter()
        .filter(|need| need.contact.y < -12.3 && need.contact.z > 6.2)
        .map(|need| need.reason)
        .collect()
}

#[test]
fn a_shelf_is_held_as_a_step_of_the_resin_it_puts_out() {
    let needs = sized_needs_for(&shelf_under_a_lean(true));
    let steps: Vec<Scalar> = needs
        .iter()
        .filter(|need| (need.contact.z - 5.0).abs() < 0.2)
        .filter_map(|need| match need.reason {
            Reason::Step { new_mm3 } => Some(new_mm3),
            _ => None,
        })
        .collect();
    assert!(!steps.is_empty(), "the shelf is held as a step");
    assert!(
        steps.iter().all(|mm3| (11.0..=12.5).contains(mm3)),
        "240 mm² of shelf in a 0.05 mm layer is 12 mm³, got {steps:?}"
    );
}

#[test]
fn a_face_leaning_past_a_steps_angle_is_held_where_the_step_reaches_it() {
    let held = under_the_lean(&sized_needs_for(&shelf_under_a_lean(true)));
    assert!(
        !held.is_empty(),
        "the 30 degree face over the shelf is held"
    );
    assert!(
        held.iter()
            .all(|reason| matches!(reason, Reason::Step { .. })),
        "as part of the step, got {held:?}"
    );
    assert!(
        under_the_lean(&sized_needs_for(&shelf_under_a_lean(false)))
            .iter()
            .all(|reason| matches!(reason, Reason::Steady)),
        "the same face with no step under it is only steadied"
    );
    assert!(
        under_the_lean(&needs_for(&shelf_under_a_lean(true))).is_empty(),
        "and a run that does not size its supports holds a step as it always has"
    );
}

/// A block 20 mm wide on the plate whose face along -y leans 15 degrees out over 30 mm
/// of rise: too gentle to sag, steep enough to steady a part on.
fn leaning_wall() -> Mesh {
    let mut wall = box_mesh(Vec3::ZERO, Vec3::new(20.0, 10.0, 30.0));
    let out = 30.0 * 15.0_f32.to_radians().tan();
    for corner in [4, 5] {
        wall.vertices[corner].y -= out;
    }
    wall
}

#[test]
fn a_gentle_slope_is_steadied_sparsely_and_only_when_sized() {
    let on_the_face = |needs: Vec<Need>| -> Vec<Need> {
        needs
            .into_iter()
            .filter(|need| need.contact.y < -0.5)
            .collect()
    };
    let steadied = on_the_face(sized_needs_for(&leaning_wall()));
    assert!(!steadied.is_empty(), "a 15 degree face is steadied on");
    assert!(
        steadied
            .iter()
            .all(|need| matches!(need.reason, Reason::Steady)),
        "and only steadied"
    );
    for (index, one) in steadied.iter().enumerate() {
        for other in &steadied[index + 1..] {
            assert!(
                one.contact.distance(other.contact) > 5.0,
                "steadying supports stand far apart: {} and {}",
                one.contact,
                other.contact
            );
        }
    }
    assert!(
        on_the_face(needs_for(&leaning_wall())).is_empty(),
        "a run that does not size its supports leaves it alone"
    );
}

#[test]
fn a_step_above_another_one_is_held_as_well() {
    // One millimetre of jut in a single layer, four above the ledge under it: too wide to bridge,
    // too far up for the supports below (ADR 0079, 0080).
    let points = supports_for(&steps(1.0, 4.0));
    let upper = points.iter().filter(|point| point.y < 0.0).count();

    // Ten millimetres of shelf at the medium preset's five millimetre spacing.
    assert!(
        upper >= 2,
        "the upper step is 10 mm of shelf and got {upper} supports"
    );
}

/// The region every face of `mesh` looking down belongs to: the underside a support
/// would otherwise be put against.
fn underside(mesh: &Mesh) -> core_geometry::Region {
    let mut region = core_geometry::Region::default();
    for face in 0..mesh.faces.len() {
        let looking_down = mesh
            .triangle(face)
            .is_some_and(|triangle| triangle.normal_unnormalized().normalize_or_zero().z < -0.9);
        region.set(face, looking_down);
    }
    region
}

#[test]
fn an_island_painted_out_of_bounds_is_left_alone() {
    let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
    let blocked = Blocked::new(
        &floating,
        &underside(&floating),
        core_geometry::Transform::default(),
    )
    .expect("the underside was painted");

    let points = generate_supports(
        &slice(&floating),
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        Some(&blocked),
        &mut |_| true,
    );

    assert!(
        points.is_empty(),
        "the only surface that needed holding up was blocked, got {} contacts",
        points.len()
    );
}

#[test]
fn a_box_standing_on_the_plate_needs_no_supports() {
    let standing = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));
    assert!(supports_for(&standing).is_empty());
}

#[test]
fn a_box_floating_over_the_plate_is_an_island_and_is_held_from_underneath() {
    let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
    let points = supports_for(&floating);

    assert!(!points.is_empty(), "an island must be held up");
    for point in &points {
        assert!(
            (0.0..=10.0).contains(&point.x) && (0.0..=10.0).contains(&point.y),
            "{point} is not under the island"
        );
        assert!(
            (point.z - 10.0).abs() < LAYER_HEIGHT_MM,
            "{point} is not on the underside of the island"
        );
    }
}

#[test]
fn an_island_is_held_at_the_spacing_the_profile_asks_for() {
    let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(30.0, 30.0, 20.0));
    let points = supports_for(&floating);
    let spacing = SampleConfig::for_profile(&profile()).spacing_mm;

    for row in 1..30 {
        for column in 1..30 {
            let probe = Vec2::new(column as Scalar, row as Scalar);
            let nearest = points
                .iter()
                .map(|point| (point.truncate() - probe).length())
                .fold(Scalar::INFINITY, Scalar::min);
            assert!(
                nearest <= spacing,
                "{probe} is {nearest} mm from the nearest support, past {spacing}"
            );
        }
    }
}

#[test]
fn a_ball_resting_on_the_plate_is_held_under_its_lower_cap() {
    let points = supports_for(&sphere(15.0, 0.0));
    assert!(
        points.len() >= 8,
        "a 30 mm ball hangs over nothing all round its lower cap, got {} supports",
        points.len()
    );

    // Everything leaning further than 45 degrees is below the ball's own equator by
    // the radius over root two, and its supports stand under that cap.
    for point in &points {
        assert!(
            point.z <= 15.0 - 15.0 / 2.0_f32.sqrt() + 1.0,
            "{point} is above the latitude that holds itself up"
        );
    }
}

#[test]
fn a_wall_leaning_inside_the_angle_holds_itself_up() {
    // A pyramid standing on its base: every face leans 45 degrees inwards.
    let mesh = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(20.0, 20.0, 0.0),
            Vec3::new(0.0, 20.0, 0.0),
            Vec3::new(10.0, 10.0, 10.0),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
        ],
    );
    assert!(supports_for(&mesh).is_empty());
}

#[test]
fn a_table_on_a_leg_is_held_around_its_coast_and_not_over_the_leg() {
    let leg = box_mesh(Vec3::new(10.0, 10.0, 0.0), Vec3::new(14.0, 14.0, 10.0));
    let table = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(24.0, 24.0, 12.0));
    let points = supports_for(&merged(&leg, &table));

    assert!(points.len() >= 4, "a 24 mm table needs holding all round");
    for point in &points {
        assert!(
            (point.z - 10.0).abs() < LAYER_HEIGHT_MM,
            "{point} is not on the underside of the table"
        );
        let over_the_leg = (10.0..=14.0).contains(&point.x) && (10.0..=14.0).contains(&point.y);
        assert!(!over_the_leg, "{point} stands where the leg already holds");
    }
}

#[test]
fn a_support_that_is_already_there_holds_what_is_around_it() {
    let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(4.0, 4.0, 14.0));
    assert!(!supports_for(&floating).is_empty(), "a bare island is held");

    let seeded = generate_supports(
        &slice(&floating),
        LAYER_HEIGHT_MM,
        &profile(),
        &[Vec3::new(2.0, 2.0, 10.0)],
        &[],
        None,
        &mut |_| true,
    );
    assert!(
        seeded.is_empty(),
        "a hand-placed support must not be doubled up on, got {seeded:?}"
    );
}

#[test]
fn a_second_run_over_what_the_first_one_placed_adds_nothing() {
    // A slab and a ball: flat undersides, where a contact sits right under the first
    // layer, and a curved one, where the layers it holds go on widening above it.
    for mesh in [
        box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(20.0, 20.0, 14.0)),
        sphere(8.0, 6.0),
    ] {
        let stack = slice(&mesh);
        let first = supports_for(&mesh);
        assert!(!first.is_empty(), "the part needs holding");

        let again = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &first,
            &[],
            None,
            &mut |_| true,
        );
        assert!(
            again.is_empty(),
            "every place is held by the {} supports already there, and the run put \
                 {} more beside them",
            first.len(),
            again.len()
        );
    }
}

#[test]
fn a_support_under_one_island_does_not_hold_the_island_over_it() {
    // Two slabs with 0.5 mm of air between them: the upper is within reach of what holds the
    // lower yet a part of its own (ADR 0081).
    let lower = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 11.0));
    let upper = box_mesh(Vec3::new(0.0, 0.0, 11.5), Vec3::new(10.0, 10.0, 12.5));
    let points = supports_for(&merged(&lower, &upper));

    let held_at = |z: Scalar| {
        points
            .iter()
            .filter(|point| (point.z - z).abs() < 0.5)
            .count()
    };
    assert!(held_at(10.0) >= 4, "the lower island is held");
    assert!(
        held_at(11.5) >= 4,
        "the upper island is a part of its own and is held on its own account, got \
             {} supports under it",
        held_at(11.5)
    );
}

/// A wedge 30 mm across and 30 mm deep hanging from its top face, its underside
/// leaning `lean_deg` from vertical and its lowest edge at z = 10, wound outwards.
fn hanging_wedge(lean_deg: Scalar) -> Mesh {
    let (run, depth) = (30.0, 15.0);
    let top = 10.0 + run / lean_deg.to_radians().tan();
    let vertices = vec![
        Vec3::new(0.0, -depth, top),
        Vec3::new(run, -depth, top),
        Vec3::new(run, -depth, 10.0),
        Vec3::new(0.0, depth, top),
        Vec3::new(run, depth, top),
        Vec3::new(run, depth, 10.0),
    ];
    let faces = vec![
        [0, 2, 1],
        [3, 4, 5],
        [0, 1, 4],
        [0, 4, 3],
        [1, 2, 5],
        [1, 5, 4],
        [0, 5, 2],
        [0, 3, 5],
    ];
    Mesh::new(vertices, faces)
}

/// Supports per square millimetre of plate under the middle of a hanging wedge.
fn held_per_mm2(lean_deg: Scalar, profile: &SupportProfile) -> Scalar {
    let points = generate_supports(
        &slice(&hanging_wedge(lean_deg)),
        LAYER_HEIGHT_MM,
        profile,
        &[],
        &[],
        None,
        &mut |_| true,
    );
    let inside = points
        .iter()
        .filter(|point| (4.0..26.0).contains(&point.x) && (-11.0..11.0).contains(&point.y))
        .count();
    inside as Scalar / (22.0 * 22.0)
}

#[test]
fn a_face_just_past_the_angle_is_held_more_sparsely_than_a_ceiling() {
    let mut profile = profile();
    profile.steep_contact_spacing_mm = 2.0 * profile.contact_spacing_mm;
    let (steep, flat) = (held_per_mm2(50.0, &profile), held_per_mm2(85.0, &profile));
    assert!(
        flat > 0.0 && steep < 0.6 * flat,
        "twice the spacing at the angle holds a steep face far more sparsely; got \
             {steep} supports per mm2 under the steep face and {flat} under the flat one"
    );
}

#[test]
fn a_needle_hanging_under_a_part_is_held_at_its_tip() {
    // Never as wide as an island anywhere along it, but far more than dust.
    let needle = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.3, 0.3, 13.0));
    let part = box_mesh(Vec3::new(-5.0, -5.0, 13.0), Vec3::new(5.0, 5.0, 15.0));
    let points = supports_for(&merged(&needle, &part));

    assert!(
        points.iter().any(|point| {
            (0.0..=0.3).contains(&point.x)
                && (0.0..=0.3).contains(&point.y)
                && (point.z - 10.0).abs() <= 2.0 * LAYER_HEIGHT_MM
        }),
        "the needle's tip at z = 10 hangs in mid-air, got {points:?}"
    );
}

#[test]
fn supports_brought_back_by_pieces_that_split_and_join_are_held_once() {
    let mut split = Held::new(1.0);
    split.add(Vec3::new(1.0, 2.0, 3.0), 1.0);
    let mut joined = Held::new(1.0);
    for _ in 0..100 {
        joined.take_in(&split, 0.0);
    }
    assert_eq!(joined.buckets.len(), 1);
}

#[test]
fn a_speck_too_small_to_print_is_left_alone() {
    // 0.0036 mm², under the least piece the printed stack names an island.
    let speck = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.06, 0.06, 10.4));
    assert!(supports_for(&speck).is_empty());
}

#[test]
fn a_speck_the_stack_names_an_island_is_held() {
    // 0.0225 mm², and dust by its resin: it never grows, but it cures on the film.
    let speck = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.15, 0.15, 10.4));
    let points = supports_for(&speck);
    assert!(
        points
            .iter()
            .any(|point| (0.0..=0.15).contains(&point.x) && (0.0..=0.15).contains(&point.y)),
        "the speck hangs in mid-air, got {points:?}"
    );
}

/// A pyramid of `side_mm` standing on its point at (`x`, `y`, 10), `tall_mm` high.
fn pyramid_on_its_point(x: Scalar, y: Scalar, side_mm: Scalar, tall_mm: Scalar) -> Mesh {
    let h = side_mm / 2.0;
    let top = 10.0 + tall_mm;
    Mesh::new(
        vec![
            Vec3::new(x, y, 10.0),
            Vec3::new(x - h, y - h, top),
            Vec3::new(x + h, y - h, top),
            Vec3::new(x + h, y + h, top),
            Vec3::new(x - h, y + h, top),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 4, 3],
            [0, 1, 4],
            [1, 2, 3],
            [1, 3, 4],
        ],
    )
}

#[test]
fn a_lobe_hanging_under_a_part_is_held_where_it_begins() {
    // Too narrow for the placement grid on its first layers, which cure on the film.
    let lobe = pyramid_on_its_point(5.33, 5.27, 1.0, 0.2);
    let part = box_mesh(Vec3::new(0.0, 0.0, 10.2), Vec3::new(10.0, 10.0, 12.0));
    let model = merged(&lobe, &part);
    let lows = crate::low_points(&model, core_geometry::Transform::default());
    let points = generate_supports(
        &slice(&model),
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &lows,
        None,
        &mut |_| true,
    );
    // The lobe's faces rise 0.4 mm a millimetre out from its point, and its first
    // layer is cut half a layer over the point.
    let surface_z = |point: &Vec3| 10.0 + 0.4 * (point.x - 5.33).abs().max((point.y - 5.27).abs());
    assert!(
        points.iter().any(|point| {
            point.z <= surface_z(point) && surface_z(point) < 10.0 + LAYER_HEIGHT_MM / 2.0
        }),
        "the lobe's point at (5.33, 5.27, 10) is its first layer, got {points:?}"
    );
}

#[test]
fn a_shallower_angle_puts_down_more_supports() {
    let ball = sphere(15.0, 0.0);
    let stack = slice(&ball);
    let loose = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &SupportProfile {
            max_overhang_deg: 60.0,
            ..profile()
        },
        &[],
        &[],
        None,
        &mut |_| true,
    );
    let strict = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &SupportProfile {
            max_overhang_deg: 30.0,
            ..profile()
        },
        &[],
        &[],
        None,
        &mut |_| true,
    );
    assert!(
        strict.len() > loose.len(),
        "30 degrees placed {} against 60 degrees' {}",
        strict.len(),
        loose.len()
    );
}

#[test]
fn a_denser_profile_puts_down_more_supports() {
    let ball = sphere(15.0, 0.0);
    let stack = slice(&ball);
    let normal = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| true,
    );
    let dense = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &SupportProfile {
            density: 3.0,
            ..profile()
        },
        &[],
        &[],
        None,
        &mut |_| true,
    );
    assert!(
        dense.len() > normal.len(),
        "three times the density put down {} against {}",
        dense.len(),
        normal.len()
    );
}

#[test]
fn a_run_that_is_stopped_before_it_starts_places_nothing() {
    let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(30.0, 30.0, 20.0));
    let stack = slice(&floating);

    let mut looked = 0;
    let stopped = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| {
            looked += 1;
            false
        },
    );
    assert!(stopped.is_empty());
    assert_eq!(
        looked, 1,
        "the run stopped at the first layer it asked about"
    );
}

#[test]
fn stopping_part_way_keeps_what_the_run_had_already_found() {
    let lower = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 12.0));
    let upper = box_mesh(Vec3::new(20.0, 0.0, 20.0), Vec3::new(30.0, 10.0, 22.0));
    let stack = slice(&merged(&lower, &upper));

    let half = stack.layers.len() / 2;
    let stopped = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |layer| layer < half,
    );

    assert!(!stopped.is_empty(), "the lower island was reached");
    for point in &stopped {
        assert!(point.x < 20.0, "{point} is on the island never reached");
    }
    let whole = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| true,
    );
    assert!(whole.len() > stopped.len());
}

#[test]
fn the_same_model_always_produces_the_same_supports() {
    let ball = sphere(15.0, 3.0);
    let stack = slice(&ball);
    let once = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| true,
    );
    let twice = generate_supports(
        &stack,
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| true,
    );
    assert_eq!(once, twice);
}

#[test]
fn an_empty_stack_produces_nothing() {
    let points = generate_supports(
        &Sliced::default(),
        LAYER_HEIGHT_MM,
        &profile(),
        &[],
        &[],
        None,
        &mut |_| true,
    );
    assert!(points.is_empty());
}
