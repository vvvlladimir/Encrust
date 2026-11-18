use core_geometry::{Bvh, Mesh, Quat, Transform, cuboid, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Sliced};
use core_supports::Reason;
use printer_profiles::SupportProfile;

use super::*;

const LAYER_MM: f32 = 0.05;

/// A body in plate coordinates with its hierarchy and stack.
struct Body {
    mesh: Mesh,
    bvh: Bvh,
    stack: Sliced,
}

impl Body {
    fn new(mesh: Mesh) -> Self {
        let settings = SliceSettings {
            layer_height: LAYER_MM,
            ..SliceSettings::default()
        };
        let stack = PlaneSliceEngine
            .slice(&mesh, &settings)
            .expect("a box cuts");
        Self {
            bvh: Bvh::build(&mesh),
            mesh,
            stack,
        }
    }

    fn underside(&self) -> Underside<'_> {
        Underside {
            stack: &self.stack,
            layer_mm: LAYER_MM,
            surface: &self.mesh,
            bvh: &self.bvh,
            blocked: None,
        }
    }

    /// Where a ray straight up from `x`, `y` first meets the body.
    fn under(&self, x: f32, y: f32) -> Vec3 {
        let from = Vec3::new(x, y, 0.0);
        let hit = self
            .bvh
            .raycast(&Ray::new(from, Vec3::Z))
            .expect("the body is over the point");
        from + Vec3::Z * hit.t
    }
}

fn light() -> SupportPlan {
    SupportPlan::of(SupportProfile::shipped("light").expect("light ships"))
}

/// A fin 0.6 mm thick and 16 mm wide, leant 30 degrees off upright so its underside looks
/// down, its foot `foot_mm` over the plate and rising 25 mm from there.
fn fin(foot_mm: f32) -> Body {
    let upright = cuboid(Vec3::ZERO, Vec3::new(16.0, 0.6, 25.0));
    let lean = Transform {
        rotation: Quat::from_rotation_x(30f32.to_radians()),
        ..Transform::default()
    };
    let leant = transform_mesh(&upright, lean);
    let bounds = leant.aabb().expect("a fin has bounds");
    let moved = Transform::from_translation(Vec3::new(
        40.0,
        40.0 - bounds.mins.y,
        foot_mm - bounds.mins.z,
    ));
    Body::new(transform_mesh(&leant, moved))
}

#[test]
fn a_fin_held_only_along_its_foot_is_given_a_contact_up_its_length() {
    let body = fin(20.0);
    let foot = body.mesh.aabb().expect("a fin has bounds").mins.z;
    let lowest = body
        .mesh
        .vertices
        .iter()
        .min_by(|a, b| a.z.total_cmp(&b.z))
        .expect("a fin has vertices");
    // Held along its foot alone, as placement holds an island that begins on an edge.
    let mut needs: Vec<Need> = [41.0, 47.0, 53.0]
        .iter()
        .map(|x| Need::new(body.under(*x, lowest.y + 0.3), Reason::POINT))
        .collect();
    let before = needs.len();
    let done =
        firm_up(body.underside(), &mut needs, &[], &light(), &mut || true).expect("the fin reads");
    assert!(
        done.steadied > 0,
        "a fin 25 mm tall on its foot sways: {done:?}"
    );
    let highest = needs[before..]
        .iter()
        .map(|need| need.contact.z)
        .fold(f32::MIN, f32::max);
    assert!(
        highest > foot + 5.0,
        "the contact added holds the fin well above its foot at {foot} mm, got {highest}"
    );
}

#[test]
fn a_low_slab_on_short_supports_is_left_alone() {
    let body = Body::new(transform_mesh(
        &cuboid(Vec3::ZERO, Vec3::new(4.0, 4.0, 1.0)),
        Transform::from_translation(Vec3::new(50.0, 50.0, 3.0)),
    ));
    let mut needs: Vec<Need> = [[50.5, 50.5], [53.5, 50.5], [50.5, 53.5], [53.5, 53.5]]
        .iter()
        .map(|[x, y]| Need::new(body.under(*x, *y), Reason::POINT))
        .collect();
    let want = needs.clone();
    let done =
        firm_up(body.underside(), &mut needs, &[], &light(), &mut || true).expect("the slab reads");
    assert_eq!(
        done,
        Firmed::default(),
        "four short supports hold a small slab still"
    );
    assert_eq!(needs, want);
}

#[test]
fn a_tall_column_gives_far_less_than_a_short_one() {
    let short = Give {
        point: 0,
        tree: 0,
        axial_per_mm: 10.0,
        lateral_per_mm: 100.0,
        turning_per_mm3: 10.0,
        trunk_mm4: 0.05,
        free_mm: 10.0,
        stands_on: None,
    };
    let tall = Give {
        lateral_per_mm: 800.0,
        free_mm: 20.0,
        ..short
    };
    let (short, tall) = (spring_of(&short), spring_of(&tall));
    assert!((short.lateral_n_per_mm / tall.lateral_n_per_mm - 8.0).abs() < 1e-9);
    // Euler: twice as long buckles under a quarter of the squeeze.
    assert!((short.buckles_at_n / tall.buckles_at_n - 4.0).abs() < 1e-9);
}
