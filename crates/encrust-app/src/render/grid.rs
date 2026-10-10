use core_geometry::Vec3;
use egui::Color32;

use crate::plate::BuildPlate;
use crate::render::vertex::LineVertex;
use crate::ui::theme;

/// One grid square is a centimetre, which is the unit people judge a print by, and every
/// fifth line is stronger, so a distance can be counted off in fives. The shader draws
/// both into the plate's own surface.
pub const GRID_SPACING_MM: f32 = 10.0;
pub const GRID_MAJOR_MM: f32 = 50.0;

/// How long each axis arrow from the plate's origin corner is, millimetres.
pub const AXIS_LENGTH_MM: f32 = 30.0;

/// The dashes of the outline drawn at the section's height, millimetres.
const DASH_MM: f32 = 3.0;
const DASH_GAP_MM: f32 = 2.0;

/// Line list for the plate's outline, the build volume and the axis arrows. The grid is
/// not here: it is drawn into the plate's surface, see `docs/design/viewport.md`.
pub fn plate_lines(plate: &BuildPlate) -> Vec<LineVertex> {
    let scene = theme::scene();
    // First, so that where X and Y run along the plate's edges they win the depth test
    // the outline drawn over them ties.
    let mut lines = Vec::new();
    push_axes(&mut lines);

    let corners = [
        [0.0, 0.0],
        [plate.x_mm, 0.0],
        [plate.x_mm, plate.y_mm],
        [0.0, plate.y_mm],
    ];
    for i in 0..corners.len() {
        let [ax, ay] = corners[i];
        let [bx, by] = corners[(i + 1) % corners.len()];
        push(&mut lines, [ax, ay, 0.0], [bx, by, 0.0], scene.plate_border);
    }

    push_volume(&mut lines, plate);
    lines
}

/// The plate's outline at `height_mm`, dashed, which is where the section cuts.
pub fn section_frame(plate: &BuildPlate, height_mm: f32) -> Vec<LineVertex> {
    let colour = theme::scene().section_frame;
    let corners = [
        Vec3::new(0.0, 0.0, height_mm),
        Vec3::new(plate.x_mm, 0.0, height_mm),
        Vec3::new(plate.x_mm, plate.y_mm, height_mm),
        Vec3::new(0.0, plate.y_mm, height_mm),
    ];
    let mut lines = Vec::new();
    for i in 0..corners.len() {
        let (from, to) = (corners[i], corners[(i + 1) % corners.len()]);
        let length = from.distance(to);
        let mut at = 0.0;
        while at < length {
            let end = (at + DASH_MM).min(length);
            lines.push(LineVertex::new(from.lerp(to, at / length), colour));
            lines.push(LineVertex::new(from.lerp(to, end / length), colour));
            at += DASH_MM + DASH_GAP_MM;
        }
    }
    lines
}

fn push_volume(lines: &mut Vec<LineVertex>, plate: &BuildPlate) {
    let volume = theme::scene().volume;
    let (x, y, z) = (plate.x_mm, plate.y_mm, plate.z_mm);
    let corners = [[0.0, 0.0], [x, 0.0], [x, y], [0.0, y]];

    for [cx, cy] in corners {
        push(lines, [cx, cy, 0.0], [cx, cy, z], volume);
    }
    for i in 0..corners.len() {
        let [ax, ay] = corners[i];
        let [bx, by] = corners[(i + 1) % corners.len()];
        push(lines, [ax, ay, z], [bx, by, z], volume);
    }
}

/// X, Y and Z from the plate's origin corner, X and Y along its edges, each in its own
/// axis colour.
fn push_axes(lines: &mut Vec<LineVertex>) {
    for (axis, colour) in [Vec3::X, Vec3::Y, Vec3::Z]
        .into_iter()
        .zip(theme::scene().plate_axis)
    {
        push(lines, [0.0; 3], (axis * AXIS_LENGTH_MM).to_array(), colour);
    }
}

fn push(lines: &mut Vec<LineVertex>, from: [f32; 3], to: [f32; 3], color: Color32) {
    lines.push(LineVertex::new(Vec3::from_array(from), color));
    lines.push(LineVertex::new(Vec3::from_array(to), color));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate(x_mm: f32, y_mm: f32) -> BuildPlate {
        BuildPlate {
            name: None,
            x_mm,
            y_mm,
            z_mm: 100.0,
        }
    }

    /// The three arrows standing in the plate's origin corner.
    const AXES: usize = 3;

    #[test]
    fn the_plate_is_its_outline_the_volume_and_the_axes() {
        // Four plate edges, four uprights and four lines closing the top of the volume.
        let lines = plate_lines(&plate(50.0, 30.0));
        assert_eq!(lines.len() / 2, 4 + 4 + 4 + AXES);
    }

    #[test]
    fn every_line_is_inside_the_build_volume() {
        let plate = plate(50.0, 30.0);
        for vertex in &plate_lines(&plate) {
            let [x, y, z] = vertex.position;
            assert!((0.0..=plate.x_mm).contains(&x));
            assert!((0.0..=plate.y_mm).contains(&y));
            assert!((0.0..=plate.z_mm).contains(&z));
        }
    }

    #[test]
    fn the_section_frame_is_dashed_around_the_plate_at_the_cut() {
        let plate = plate(50.0, 30.0);
        let frame = section_frame(&plate, 12.5);
        assert!(frame.iter().all(|vertex| vertex.position[2] == 12.5));

        // A 50 mm side holds ten dashes of 3 mm with 2 mm between them, a 30 mm side six:
        // a dash every 5 mm all the way round, so the outline is 160 mm of dashes and gaps.
        assert_eq!(frame.len() / 2, 10 + 6 + 10 + 6);
        let drawn: f32 = frame
            .chunks(2)
            .map(|dash| {
                Vec3::from_array(dash[0].position).distance(Vec3::from_array(dash[1].position))
            })
            .sum();
        assert!(
            (drawn - 32.0 * DASH_MM).abs() < 1e-3,
            "three fifths of the outline is drawn, got {drawn} mm"
        );
    }
}
