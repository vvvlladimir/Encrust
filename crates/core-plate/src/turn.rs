use core_geometry::{Quat, Scalar, Transform, Vec2, Vec3, convex_hull, fitting_turn};

use crate::arrange::{ArrangeSettings, PlateArea};
use crate::spot::Rect;

/// The least turn about the vertical through its middle fitting a part in `rect` onto `area`
/// where no slide does; `None` if none needed or none fits. `outline` is read only then.
pub fn turned_onto(
    rect: Rect,
    outline: impl FnOnce() -> Vec<Vec2>,
    area: PlateArea,
    settings: &ArrangeSettings,
) -> Option<Transform> {
    if (rect.max - rect.min)
        .cmple(area.high_mm - area.low_mm)
        .all()
    {
        return None;
    }
    let turn = turn_onto(&outline(), area, settings).filter(|turn| *turn != 0.0)?;
    let centre = ((rect.min + rect.max) / 2.0).extend(0.0);
    let rotation = Quat::from_rotation_z(turn);
    Some(Transform::from_scale_rotation_translation(
        Vec3::ONE,
        rotation,
        centre - rotation * centre,
    ))
}

/// The least turn about the vertical, radians counter-clockwise, bringing `outline` (plate mm)
/// onto `area`, clear of the margin before flush to the edge; `None` where none does (ADR 0418).
fn turn_onto(outline: &[Vec2], area: PlateArea, settings: &ArrangeSettings) -> Option<Scalar> {
    let hull = convex_hull(outline);
    let whole = area.high_mm - area.low_mm;
    let inner = whole - Vec2::splat(2.0 * settings.margin_mm.max(0.0));
    if fitting_turn(&hull, whole) == Some(0.0) {
        return Some(0.0);
    }
    fitting_turn(&hull, inner).or_else(|| fitting_turn(&hull, whole))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Vec3, cuboid};

    /// The outline of a box `size` across standing at the plate's origin.
    fn outline(size: Vec3) -> Vec<Vec2> {
        cuboid(Vec3::ZERO, size)
            .vertices
            .iter()
            .map(|vertex| vertex.truncate())
            .collect()
    }

    fn area(width_mm: Scalar, depth_mm: Scalar) -> PlateArea {
        PlateArea::whole(Vec2::new(width_mm, depth_mm))
    }

    #[test]
    fn a_part_that_fits_is_not_turned() {
        let turn = turn_onto(
            &outline(Vec3::new(40.0, 20.0, 5.0)),
            area(90.0, 60.0),
            &ArrangeSettings::default(),
        );
        assert_eq!(turn, Some(0.0));
    }

    #[test]
    fn a_part_that_fits_only_flush_is_slid_rather_than_turned() {
        // 88 mm fits the 90 mm plate but not its 2 mm margins either side.
        let turn = turn_onto(
            &outline(Vec3::new(88.0, 4.0, 5.0)),
            area(90.0, 90.0),
            &ArrangeSettings::default(),
        );
        assert_eq!(turn, Some(0.0));
    }

    #[test]
    fn a_part_deeper_than_the_plate_is_turned_the_least_that_fits_it() {
        let part = outline(Vec3::new(20.0, 100.0, 5.0));
        let turn = turn_onto(&part, area(140.0, 80.0), &ArrangeSettings::default())
            .expect("100 by 20 fits a 140 by 80 plate turned");
        let rotation = Vec2::from_angle(turn);
        let (low, high) = part.iter().map(|point| rotation.rotate(*point)).fold(
            (Vec2::INFINITY, Vec2::NEG_INFINITY),
            |(low, high), point| (low.min(point), high.max(point)),
        );
        let size = high - low;
        assert!(
            size.x <= 136.0 + 1e-2 && size.y <= 76.0 + 1e-2,
            "inside 140 by 80 less 2 mm a side, got {size}"
        );
        assert!(
            turn.abs() < std::f32::consts::FRAC_PI_2 - 0.1,
            "short of the quarter turn a slant fits in, got {} degrees",
            turn.to_degrees()
        );
    }

    #[test]
    fn a_turned_part_keeps_the_middle_of_where_it_stood() {
        let part = outline(Vec3::new(20.0, 100.0, 5.0));
        let rect = Rect {
            min: Vec2::ZERO,
            max: Vec2::new(20.0, 100.0),
        };
        let moved = turned_onto(
            rect,
            || part,
            area(140.0, 80.0),
            &ArrangeSettings::default(),
        )
        .expect("no slide fits 100 mm into 80");
        let middle = moved
            .to_matrix()
            .transform_point3(Vec3::new(10.0, 50.0, 3.0));
        assert!(
            (middle - Vec3::new(10.0, 50.0, 3.0)).length() < 1e-3,
            "the middle stays at (10, 50, 3), got {middle}"
        );
    }

    #[test]
    fn a_part_a_slide_fits_is_not_turned_nor_its_outline_read() {
        let rect = Rect {
            min: Vec2::splat(-30.0),
            max: Vec2::splat(10.0),
        };
        let moved = turned_onto(
            rect,
            || unreachable!("a slide fits 40 by 40 onto 90 by 60"),
            area(90.0, 60.0),
            &ArrangeSettings::default(),
        );
        assert_eq!(moved, None);
    }

    #[test]
    fn a_part_wider_than_any_turn_allows_is_not_turned() {
        let turn = turn_onto(
            &outline(Vec3::new(95.0, 95.0, 5.0)),
            area(90.0, 90.0),
            &ArrangeSettings::default(),
        );
        assert_eq!(turn, None);
    }
}
