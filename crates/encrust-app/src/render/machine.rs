use core_geometry::Vec3;
use egui::Color32;

use crate::plate::BuildPlate;
use crate::render::vertex::BodyVertex;
use crate::ui::theme;

/// Thickness of the build platform deck, millimetres.
const DECK_MM: f32 = 8.0;

/// How far the deck stands out past the plate on each side, millimetres. Wide enough to
/// carry the word written on its front lip.
pub const LIP_MM: f32 = 12.0;

/// How far the deck's underside is drawn in from its top edge: the draft that tells the
/// top of the deck from its sides under a light that comes from one direction.
const DRAFT_MM: f32 = 3.0;

/// The arm the deck hangs from, millimetres. It runs away from the print rather than past
/// it, which on a machine that cures downwards puts it under the deck. See ADR 0108.
const ARM_WIDTH_MM: f32 = 36.0;
const ARM_DEPTH_MM: f32 = 14.0;
const ARM_LENGTH_MM: f32 = 72.0;
/// How far the free end of the arm is drawn in: a column is thickest where it is held.
const ARM_TAPER_MM: f32 = 1.5;

/// The levelling knob on the front of the arm.
const KNOB_RADIUS_MM: f32 = 8.0;
const KNOB_LENGTH_MM: f32 = 7.0;
const KNOB_SIDES: usize = 12;

/// Triangle list for the build platform the models stand on, and the arm it hangs from.
///
/// The arm comes first so that the deck's translucent top blends over it rather than the
/// other way round: the arm is behind the platform and has to read as behind it. The deck
/// is a face over the whole plate, drawn after the grid and losing every depth tie to it,
/// so the plate reads as a surface while its lines stay crisp. Everything is under the
/// plate, so nothing the machine draws can stand in the build volume.
pub fn machine_faces(plate: &BuildPlate) -> Vec<BodyVertex> {
    let scene = theme::scene();
    let mut faces = Vec::new();
    arm(&mut faces, plate, scene.platform);
    deck(&mut faces, plate, scene.platform);
    faces
}

fn deck(faces: &mut Vec<BodyVertex>, plate: &BuildPlate, colour: Color32) {
    let (left, front) = (-LIP_MM, -LIP_MM);
    let (right, back) = (plate.x_mm + LIP_MM, plate.y_mm + LIP_MM);
    floor(faces, [left, front, right, back], 0.0, colour);
    walls(
        faces,
        [left, front, right, back],
        (0.0, -DECK_MM),
        DRAFT_MM,
        colour,
    );
    floor(
        faces,
        drawn_in([left, front, right, back], DRAFT_MM),
        -DECK_MM,
        colour,
    );
}

fn arm(faces: &mut Vec<BodyVertex>, plate: &BuildPlate, colour: Color32) {
    let middle = plate.x_mm / 2.0;
    let front = plate.y_mm;
    let sides = [
        middle - ARM_WIDTH_MM / 2.0,
        front,
        middle + ARM_WIDTH_MM / 2.0,
        front + ARM_DEPTH_MM,
    ];
    let end = -(DECK_MM + ARM_LENGTH_MM);
    walls(faces, sides, (-DECK_MM, end), ARM_TAPER_MM, colour);
    floor(faces, drawn_in(sides, ARM_TAPER_MM), end, colour);
    knob(
        faces,
        Vec3::new(middle, front, end + ARM_LENGTH_MM * 0.45),
        colour,
    );
}

/// The rectangle `sides` pulled in by `draft` on every edge.
fn drawn_in(sides: [f32; 4], draft: f32) -> [f32; 4] {
    let [left, front, right, back] = sides;
    [left + draft, front + draft, right - draft, back - draft]
}

/// A prism lying on its side against the arm, which is what a levelling knob reads as at
/// this size without being modelled as one.
fn knob(faces: &mut Vec<BodyVertex>, against: Vec3, colour: Color32) {
    let rim = |step: usize, y: f32| {
        let angle = std::f32::consts::TAU * step as f32 / KNOB_SIDES as f32;
        Vec3::new(
            against.x + KNOB_RADIUS_MM * angle.cos(),
            y,
            against.z + KNOB_RADIUS_MM * angle.sin(),
        )
    };
    let face = against.y - KNOB_LENGTH_MM;
    for step in 0..KNOB_SIDES {
        quad(
            faces,
            [
                rim(step, against.y),
                rim(step + 1, against.y),
                rim(step + 1, face),
                rim(step, face),
            ],
            colour,
        );
        quad(
            faces,
            [
                Vec3::new(against.x, face, against.z),
                rim(step, face),
                rim(step + 1, face),
                Vec3::new(against.x, face, against.z),
            ],
            colour,
        );
    }
}

/// Four upright faces around `[left, front, right, back]`, from `heights.0` down to
/// `heights.1`, the lower edge drawn in by `draft` on every side.
fn walls(
    faces: &mut Vec<BodyVertex>,
    sides: [f32; 4],
    heights: (f32, f32),
    draft: f32,
    colour: Color32,
) {
    let [left, front, right, back] = sides;
    let (top, bottom) = heights;
    let corners = [[left, front], [right, front], [right, back], [left, back]];
    let pulled = |[x, y]: [f32; 2]| {
        [
            if x < right { x + draft } else { x - draft },
            if y < back { y + draft } else { y - draft },
        ]
    };
    for index in 0..corners.len() {
        let (a, b) = (corners[index], corners[(index + 1) % corners.len()]);
        let (pulled_a, pulled_b) = (pulled(a), pulled(b));
        quad(
            faces,
            [
                Vec3::new(a[0], a[1], top),
                Vec3::new(b[0], b[1], top),
                Vec3::new(pulled_b[0], pulled_b[1], bottom),
                Vec3::new(pulled_a[0], pulled_a[1], bottom),
            ],
            colour,
        );
    }
}

/// One flat face filling `[left, front, right, back]` at `height`.
fn floor(faces: &mut Vec<BodyVertex>, sides: [f32; 4], height: f32, colour: Color32) {
    let [left, front, right, back] = sides;
    quad(
        faces,
        [
            Vec3::new(left, front, height),
            Vec3::new(right, front, height),
            Vec3::new(right, back, height),
            Vec3::new(left, back, height),
        ],
        colour,
    );
}

/// Two triangles with the normal their own winding gives them. Both sides are lit, so a
/// face wound the other way is shaded the same rather than left black.
pub(crate) fn quad(faces: &mut Vec<BodyVertex>, corners: [Vec3; 4], colour: Color32) {
    let [a, b, c, d] = corners;
    let normal = (b - a).cross(c - a).normalize_or_zero();
    for point in [a, b, c, a, c, d] {
        faces.push(BodyVertex::new(point, normal, colour));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate() -> BuildPlate {
        BuildPlate {
            name: None,
            x_mm: 150.0,
            y_mm: 80.0,
            z_mm: 165.0,
        }
    }

    #[test]
    fn nothing_the_machine_draws_stands_in_the_build_volume() {
        for vertex in machine_faces(&plate()) {
            let [x, y, z] = vertex.position;
            assert!(z <= 0.0, "{x},{y},{z} stands above the plate");
        }
    }

    #[test]
    fn the_arm_hangs_behind_the_plate_and_under_the_deck() {
        let plate = plate();
        let faces = machine_faces(&plate);
        let lowest = faces
            .iter()
            .map(|vertex| vertex.position[2])
            .fold(f32::INFINITY, f32::min);
        assert!((lowest + DECK_MM + ARM_LENGTH_MM).abs() < 1e-5);

        for vertex in faces {
            let [_, y, z] = vertex.position;
            assert!(
                z >= -DECK_MM || y > plate.y_mm / 2.0,
                "{y},{z} hangs under the print rather than behind it"
            );
        }
    }
}
