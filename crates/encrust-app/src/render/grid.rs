use core_geometry::Vec3;
use egui::Color32;

use crate::plate::Plate;
use crate::render::vertex::LineVertex;
use crate::ui::theme;

/// One grid square is a centimetre, which is the unit people judge a print by.
pub const GRID_SPACING_MM: f32 = 10.0;

/// Line list for the plate surface, the build volume outline, and the centimetre grid
/// when it is switched on.
///
/// The two axes are drawn along the plate edges that start at the origin, so the
/// coordinates in the Settings panel can be read straight off the picture.
pub fn plate_lines(plate: &Plate, grid: bool) -> Vec<LineVertex> {
    let scene = theme::scene();
    let [axis_x, axis_y] = scene.plate_axis;
    let mut lines = Vec::new();

    if grid {
        for x in interior_offsets(plate.x_mm) {
            push(&mut lines, [x, 0.0, 0.0], [x, plate.y_mm, 0.0], scene.grid);
        }
        for y in interior_offsets(plate.y_mm) {
            push(&mut lines, [0.0, y, 0.0], [plate.x_mm, y, 0.0], scene.grid);
        }
    }

    push(&mut lines, [0.0, 0.0, 0.0], [plate.x_mm, 0.0, 0.0], axis_x);
    push(&mut lines, [0.0, 0.0, 0.0], [0.0, plate.y_mm, 0.0], axis_y);
    push(
        &mut lines,
        [plate.x_mm, 0.0, 0.0],
        [plate.x_mm, plate.y_mm, 0.0],
        scene.plate_border,
    );
    push(
        &mut lines,
        [0.0, plate.y_mm, 0.0],
        [plate.x_mm, plate.y_mm, 0.0],
        scene.plate_border,
    );

    push_volume(&mut lines, plate);
    lines
}

/// Grid offsets strictly inside the plate; the edges are drawn as the border instead.
fn interior_offsets(extent_mm: f32) -> impl Iterator<Item = f32> {
    let count = (extent_mm / GRID_SPACING_MM).ceil() as i32;
    (1..count).map(|i| i as f32 * GRID_SPACING_MM)
}

fn push_volume(lines: &mut Vec<LineVertex>, plate: &Plate) {
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

fn push(lines: &mut Vec<LineVertex>, from: [f32; 3], to: [f32; 3], color: Color32) {
    lines.push(LineVertex::new(Vec3::from_array(from), color));
    lines.push(LineVertex::new(Vec3::from_array(to), color));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate(x_mm: f32, y_mm: f32) -> Plate {
        Plate {
            name: None,
            x_mm,
            y_mm,
            z_mm: 100.0,
        }
    }

    #[test]
    fn a_plate_of_whole_centimetres_has_the_expected_line_count() {
        // 50 x 30 mm: four interior lines along X, two along Y, four plate edges,
        // four uprights and four lines closing the top of the volume.
        let lines = plate_lines(&plate(50.0, 30.0), true);
        assert_eq!(lines.len() / 2, 4 + 2 + 4 + 4 + 4);
    }

    #[test]
    fn switching_the_grid_off_leaves_the_plate_and_the_volume() {
        // The four plate edges, four uprights and four lines closing the top.
        let lines = plate_lines(&plate(50.0, 30.0), false);
        assert_eq!(lines.len() / 2, 4 + 4 + 4);
    }

    #[test]
    fn a_partial_last_square_still_gets_its_grid_line() {
        // 25 mm fits two whole squares and a remainder, so lines land at 10 and 20.
        let interior: Vec<f32> = interior_offsets(25.0).collect();
        assert_eq!(interior, vec![10.0, 20.0]);
    }

    #[test]
    fn a_plate_smaller_than_one_square_has_no_interior_lines() {
        assert_eq!(interior_offsets(6.0).count(), 0);
    }

    #[test]
    fn every_line_is_inside_the_build_volume() {
        let plate = plate(50.0, 30.0);
        for vertex in plate_lines(&plate, true) {
            let [x, y, z] = vertex.position;
            assert!((0.0..=plate.x_mm).contains(&x));
            assert!((0.0..=plate.y_mm).contains(&y));
            assert!((0.0..=plate.z_mm).contains(&z));
        }
    }
}
