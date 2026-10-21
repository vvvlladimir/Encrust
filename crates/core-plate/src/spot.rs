use core_geometry::{Scalar, Vec2};

use crate::arrange::ArrangeSettings;

/// A box across the plate, millimetres: what a model, its supports or its raft cover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

/// The centre nearest `toward` for a part `size_mm` across on a plate `plate_mm` across,
/// `clearance_mm` clear of every one of `taken` and `margin_mm` in from the edge, or `None`
/// where no room is left on the plate.
///
/// Exact rather than on a grid: the nearest free point is `toward` itself or lies on a side
/// of a box grown by the part, so only those sides' lines and their crossings are tried.
pub fn nearest_free(
    size_mm: Vec2,
    taken: &[Rect],
    toward: Vec2,
    plate_mm: Vec2,
    settings: &ArrangeSettings,
) -> Option<Vec2> {
    let half = size_mm / 2.0;
    let room = Rect {
        min: Vec2::splat(settings.margin_mm) + half,
        max: plate_mm - Vec2::splat(settings.margin_mm) - half,
    };
    if room.min.cmpgt(room.max).any() {
        return None;
    }
    let reach = half + Vec2::splat(settings.clearance_mm);
    let grown: Vec<Rect> = taken
        .iter()
        .map(|rect| Rect {
            min: rect.min - reach,
            max: rect.max + reach,
        })
        .collect();
    let lines = |axis: fn(Vec2) -> Scalar| -> Vec<Scalar> {
        let mut lines = vec![axis(toward), axis(room.min), axis(room.max)];
        lines.extend(
            grown
                .iter()
                .flat_map(|rect| [axis(rect.min), axis(rect.max)]),
        );
        lines
    };
    let (xs, ys) = (lines(|at| at.x), lines(|at| at.y));
    xs.iter()
        .flat_map(|x| ys.iter().map(move |y| Vec2::new(*x, *y)))
        .map(|at| at.clamp(room.min, room.max))
        .filter(|at| grown.iter().all(|rect| !strictly_inside(rect, *at)))
        .min_by(|a, b| {
            a.distance_squared(toward)
                .total_cmp(&b.distance_squared(toward))
        })
}

fn strictly_inside(rect: &Rect, at: Vec2) -> bool {
    at.cmpgt(rect.min).all() && at.cmplt(rect.max).all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> ArrangeSettings {
        ArrangeSettings {
            clearance_mm: 3.0,
            margin_mm: 2.0,
            ..ArrangeSettings::default()
        }
    }

    fn rect(min: (Scalar, Scalar), max: (Scalar, Scalar)) -> Rect {
        Rect {
            min: Vec2::new(min.0, min.1),
            max: Vec2::new(max.0, max.1),
        }
    }

    const PLATE: Vec2 = Vec2::new(100.0, 60.0);

    #[test]
    fn a_part_with_room_where_it_is_asked_stays_there() {
        let at = nearest_free(
            Vec2::splat(4.0),
            &[],
            Vec2::new(30.0, 30.0),
            PLATE,
            &settings(),
        );
        assert_eq!(at, Some(Vec2::new(30.0, 30.0)));
    }

    #[test]
    fn a_part_asked_onto_a_model_moves_to_its_nearest_side() {
        // A model from x 20 to 40; the part asked 2 mm left of its right side.
        let model = rect((20.0, 10.0), (40.0, 50.0));
        let at = nearest_free(
            Vec2::splat(4.0),
            &[model],
            Vec2::new(38.0, 30.0),
            PLATE,
            &settings(),
        )
        .expect("the plate has room");
        // Its right side, the 3 mm clearance and half the 4 mm part: x = 45.
        assert!(
            (at - Vec2::new(45.0, 30.0)).length() < 1e-5,
            "the part stands 3 mm right of the model, got {at}"
        );
    }

    #[test]
    fn a_part_asked_off_the_plate_is_brought_back_inside_its_margin() {
        let at = nearest_free(
            Vec2::splat(4.0),
            &[],
            Vec2::new(120.0, 30.0),
            PLATE,
            &settings(),
        );
        // 100 mm less the 2 mm margin and half the part.
        assert_eq!(at, Some(Vec2::new(96.0, 30.0)));
    }

    #[test]
    fn a_part_between_two_models_takes_the_gap_only_where_it_fits() {
        // Two models 8 mm apart: a 4 mm part needs 3 + 4 + 3 = 10 mm, so it goes round.
        let left = rect((10.0, 10.0), (40.0, 50.0));
        let right = rect((48.0, 10.0), (80.0, 50.0));
        let at = nearest_free(
            Vec2::splat(4.0),
            &[left, right],
            Vec2::new(44.0, 30.0),
            PLATE,
            &settings(),
        )
        .expect("the plate has room");
        assert!(
            at.y <= 10.0 - 5.0 + 1e-5 || at.y >= 50.0 + 5.0 - 1e-5,
            "the part clears both models above or below them, got {at}"
        );
    }

    #[test]
    fn a_plate_with_no_room_left_says_so() {
        let everything = rect((0.0, 0.0), (100.0, 60.0));
        let at = nearest_free(
            Vec2::splat(4.0),
            &[everything],
            Vec2::new(50.0, 30.0),
            PLATE,
            &settings(),
        );
        assert_eq!(at, None);
    }
}
