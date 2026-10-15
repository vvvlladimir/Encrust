//! An orientation chosen by printing the search's finalists, through the public entry
//! points alone.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::path::Path;

use core_engine::{EngineError, Trial, orient_proven, print_out, prove};
use core_geometry::{Mesh, Quat, Vec3};
use core_plate::{OrientGoal, OrientSettings, Printed, Ruling, finalists};
use printer_profiles::{MaterialProfile, PrinterProfile, SupportProfile};

/// Panel of 600 x 400 pixels at a 0.1 mm pitch: 60 x 40 mm of build area.
const PROFILE: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 600
height_px = 400
width_mm = 60.0
height_mm = 40.0

[build_volume]
x = 60.0
y = 40.0
z = 80.0
"#;

/// What every pose is printed with: the test panel, the default resin, medium supports.
struct Bench {
    printer: PrinterProfile,
    material: MaterialProfile,
    support: SupportProfile,
}

impl Bench {
    fn new() -> Self {
        Self {
            printer: PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
                .expect("the inline profile parses"),
            material: MaterialProfile::default(),
            support: SupportProfile::medium(),
        }
    }

    fn trial(&self) -> Trial<'_> {
        Trial {
            printer: &self.printer,
            material: &self.material,
            support: &self.support,
            raster_window: 2,
        }
    }

    fn settings(&self, goal: OrientGoal) -> OrientSettings {
        OrientSettings {
            goal,
            max_overhang_deg: self.support.max_overhang_deg,
            contact_spacing_mm: self.support.contact_spacing_mm,
            on_supports: true,
            rest_on_plate: true,
            ..OrientSettings::default()
        }
    }
}

/// Boxes laid into one mesh, each twelve triangles wound outward.
fn boxes(parts: &[(Vec3, Vec3)]) -> Mesh {
    let mut mesh = Mesh::default();
    for (mins, maxs) in parts {
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend((0..8).map(|corner| {
            let pick = |axis: usize| {
                if corner >> axis & 1 == 0 {
                    mins[axis]
                } else {
                    maxs[axis]
                }
            };
            Vec3::new(pick(0), pick(1), pick(2))
        }));
        let faces = [
            [0, 2, 1],
            [1, 2, 3],
            [4, 5, 6],
            [5, 7, 6],
            [0, 1, 4],
            [1, 5, 4],
            [2, 6, 3],
            [3, 6, 7],
            [0, 4, 2],
            [2, 4, 6],
            [1, 3, 5],
            [3, 7, 5],
        ];
        mesh.faces
            .extend(faces.iter().map(|face| face.map(|index| index + offset)));
    }
    mesh
}

/// A 4 mm stem under a 20 by 20 cap: as drawn the cap hangs, upside down it rests.
fn mushroom() -> Mesh {
    boxes(&[
        (Vec3::new(8.0, 8.0, 0.0), Vec3::new(12.0, 12.0, 15.0)),
        (Vec3::new(0.0, 0.0, 15.0), Vec3::new(20.0, 20.0, 18.0)),
    ])
}

#[test]
fn a_box_set_on_the_plate_prints_with_nothing_under_it_and_nothing_hanging() {
    let bench = Bench::new();
    let mesh = boxes(&[(Vec3::ZERO, Vec3::new(10.0, 10.0, 5.0))]);
    let printed = Printed::solid(&mesh);

    let bare = print_out(&printed, Quat::IDENTITY, true, &bench.trial()).expect("a box prints");
    assert_eq!((bare.contacts, bare.hanging, bare.islands), (0, 0, 0));
    // 5 mm at the default resin's 0.05 mm a layer.
    assert_eq!(bare.layers, 100);
    assert!(
        (bare.stack_mm - 5.0).abs() < 1e-3,
        "the stack is the box's 5 mm, got {}",
        bare.stack_mm
    );

    let lifted = print_out(&printed, Quat::IDENTITY, false, &bench.trial()).expect("a box prints");
    assert!(
        lifted.contacts > 0,
        "lifted off the plate, its underside is held"
    );
    assert!(
        lifted.layers > bare.layers,
        "and the supports add their height"
    );
    assert!(
        lifted.stack_mm > bare.stack_mm,
        "and to the stack the time follows"
    );
}

#[test]
fn a_cap_hanging_over_its_stem_on_the_plate_is_held_from_the_plate() {
    let bench = Bench::new();
    let mesh = mushroom();
    let as_drawn = print_out(&Printed::solid(&mesh), Quat::IDENTITY, true, &bench.trial())
        .expect("a mushroom prints");
    assert!(as_drawn.contacts > 0, "the cap's underside is held");
    assert_eq!(
        as_drawn.hanging, 0,
        "and every contact has room for a column"
    );
}

#[test]
fn a_mushroom_is_proven_to_rest_on_its_cap() {
    let bench = Bench::new();
    let mesh = mushroom();
    let settings = bench.settings(OrientGoal::FewestSupports);

    let proven = orient_proven(&Printed::solid(&mesh), &settings, &bench.trial())
        .expect("a mushroom can be oriented");
    assert!(proven.oriented.on_plate, "cap down it needs no supports");
    assert_eq!((proven.printout.contacts, proven.printout.hanging), (0, 0));
    let stem = proven.oriented.rotation * Vec3::Z;
    assert!(
        stem.z < -0.99,
        "the stem points up from the cap, got {stem}"
    );
}

#[test]
fn a_table_stays_on_the_plate_held_only_under_its_top() {
    let bench = Bench::new();
    // A 2 mm foot and a 2 mm top, both 24 mm square, on a 4 mm leg: it sits on a whole
    // face either way up, and either way the other slab hangs all round the leg.
    let mesh = boxes(&[
        (Vec3::ZERO, Vec3::new(24.0, 24.0, 2.0)),
        (Vec3::new(10.0, 10.0, 2.0), Vec3::new(14.0, 14.0, 14.0)),
        (Vec3::new(0.0, 0.0, 14.0), Vec3::new(24.0, 24.0, 16.0)),
    ]);
    let settings = OrientSettings {
        max_tilt_deg: Some(0.0),
        ..bench.settings(OrientGoal::FewestSupports)
    };
    let printed = Printed::solid(&mesh);

    let lifted =
        print_out(&printed, Quat::IDENTITY, false, &bench.trial()).expect("a table prints");
    let seated = print_out(&printed, Quat::IDENTITY, true, &bench.trial()).expect("a table prints");
    assert!(seated.contacts > 0, "the top hangs round the leg");
    assert!(
        seated.supports_ml < lifted.supports_ml,
        "the plate holds the foot: {} ml against {} ml lifted",
        seated.supports_ml,
        lifted.supports_ml
    );

    let found = finalists(&printed, &settings, usize::MAX).expect("it has finalists");
    let tried = prove(&printed, &settings, &found, &bench.trial()).expect("its finalists print");
    assert!(
        matches!(tried[0].ruling, Ruling::Chosen | Ruling::Stayed),
        "the first tried is the choice, got {:?}",
        tried[0].ruling
    );
    let proven = tried[0].proven;
    assert!(proven.oriented.on_plate, "it stays standing on its foot");
    assert!(proven.printout.contacts > 0, "with supports under its top");
}

/// A ball of `radius` mm, `rings` bands of `rings * 2` quads, standing clear of z = 0.
fn ball(radius: f32, rings: u32) -> Mesh {
    let segments = rings * 2;
    let mut vertices = vec![Vec3::new(0.0, 0.0, radius)];
    for ring in 1..rings {
        let theta = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as f32 / segments as f32;
            vertices.push(Vec3::new(
                radius * theta.sin() * phi.cos(),
                radius * theta.sin() * phi.sin(),
                radius * theta.cos(),
            ));
        }
    }
    let south = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, 0.0, -radius));
    let at = |ring: u32, segment: u32| 1 + (ring - 1) * segments + segment % segments;
    let mut faces = Vec::new();
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
        faces.push([at(rings - 1, segment), south, at(rings - 1, segment + 1)]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            faces.push([
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
            ]);
            faces.push([
                at(ring, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ]);
        }
    }
    Mesh::new(vertices, faces)
}

#[test]
fn a_trial_counts_the_contacts_the_print_stands_on() {
    let bench = Bench::new();
    let mesh = ball(12.0, 48);
    let settings = bench.settings(OrientGoal::FewestSupports);
    let printed = Printed::solid(&mesh);

    let found = finalists(&printed, &settings, usize::MAX).expect("a ball has finalists");
    let tried = prove(&printed, &settings, &found, &bench.trial()).expect("a ball prints");
    for pose in tried
        .iter()
        .map(|tried| tried.proven)
        .filter(|pose| !pose.oriented.on_plate)
    {
        let print = print_out(&printed, pose.oriented.rotation, false, &bench.trial())
            .expect("a ball prints");
        assert_eq!(
            (pose.printout.contacts, pose.printout.hanging),
            (print.contacts, print.hanging),
            "placement cut coarser than the print puts down other contacts"
        );
    }
}

#[test]
fn the_fewest_supports_leave_a_cup_standing_on_its_floor() {
    let bench = Bench::new();
    // A 30 mm box open at the top: on its floor it needs no support, and seals a cup.
    let mesh = boxes(&[
        (Vec3::ZERO, Vec3::new(30.0, 30.0, 2.0)),
        (Vec3::new(0.0, 0.0, 2.0), Vec3::new(2.0, 30.0, 20.0)),
        (Vec3::new(28.0, 0.0, 2.0), Vec3::new(30.0, 30.0, 20.0)),
        (Vec3::new(2.0, 0.0, 2.0), Vec3::new(28.0, 2.0, 20.0)),
        (Vec3::new(2.0, 28.0, 2.0), Vec3::new(28.0, 30.0, 20.0)),
    ]);
    let settings = bench.settings(OrientGoal::FewestSupports);

    let proven = orient_proven(&Printed::solid(&mesh), &settings, &bench.trial())
        .expect("a box can be oriented");
    let floor = proven.oriented.rotation * Vec3::NEG_Z;
    assert!(
        floor.z < -0.99,
        "no support beats any, cup or not, got the floor at {floor}"
    );
    assert_eq!(proven.printout.supports_ml, 0.0);
}

#[test]
fn a_mesh_with_no_faces_has_no_orientation_to_prove() {
    let bench = Bench::new();
    let settings = bench.settings(OrientGoal::FewestSupports);
    let error = orient_proven(&Printed::solid(&Mesh::default()), &settings, &bench.trial())
        .expect_err("an empty mesh has no orientation");
    assert!(matches!(error, EngineError::Orient(_)));
}
