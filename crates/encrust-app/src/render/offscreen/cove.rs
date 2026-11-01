//! The render view's room: the wall of the cove met where it stands.

use super::SIZE;
use super::marks::Marks;
use super::paint::paint;
use crate::camera::OrbitCamera;
use crate::render::gpu::{Desk, Look};

/// Seen level across the table, the wall rises from the curve to the top of the frame, and
/// falls off evenly towards it: a ray marched short of the wall would light it as the floor.
#[test]
fn the_cove_wall_darkens_evenly_up_the_frame() {
    let look = Look {
        pivot_mm: 300.0,
        desk: Desk {
            rect: [-200.0, -150.0, 200.0, 150.0],
            mat_mm: [400.0, 300.0],
        },
        ..Look::default()
    };
    let camera = OrbitCamera {
        pitch_rad: 0.05,
        ..OrbitCamera::default()
    };
    let marks = Marks {
        look: Some(look),
        ..Marks::default()
    };
    let Some(pixels) = paint(camera, None, &[], &[], marks) else {
        return;
    };
    let middle = (SIZE / 2) as usize;
    let brightness = |row: usize| {
        let at = (row * SIZE as usize + middle) * 4;
        pixels[at..at + 3]
            .iter()
            .map(|c| u32::from(*c))
            .sum::<u32>()
    };
    // The wall and the curve fill the frame's top 360 rows from this camera.
    for row in 0..360 {
        assert!(
            brightness(row + 1) + 3 >= brightness(row),
            "row {} is darker than row {row} above it: {} against {}",
            row + 1,
            brightness(row + 1),
            brightness(row)
        );
    }
}
