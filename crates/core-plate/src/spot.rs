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
/// of a box grown by the part, so only those sides' lines are walked, nearest first.
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
    let goal = toward.clamp(room.min, room.max);
    if grown.iter().all(|rect| !strictly_inside(rect, goal)) {
        return Some(goal);
    }
    let turned: Vec<Rect> = grown.iter().map(Rect::turned).collect();
    let columns = nearest_on_columns(toward, &room, &grown, Scalar::INFINITY);
    let within = columns.map_or(Scalar::INFINITY, |at| at.distance_squared(toward));
    let rows = nearest_on_columns(swapped(toward), &room.turned(), &turned, within);
    rows.map(swapped).or(columns)
}

impl Rect {
    /// The box mirrored across the diagonal, so its rows are walked as columns.
    fn turned(&self) -> Self {
        Self {
            min: swapped(self.min),
            max: swapped(self.max),
        }
    }
}

/// The free point nearest `toward` on a line `x = constant` along a side of `room` or of one
/// of `grown`, where it is nearer than `within`, squared millimetres.
fn nearest_on_columns(toward: Vec2, room: &Rect, grown: &[Rect], within: Scalar) -> Option<Vec2> {
    let mut xs: Vec<Scalar> = [room.min.x, room.max.x]
        .into_iter()
        .chain(grown.iter().flat_map(|rect| [rect.min.x, rect.max.x]))
        .filter(|x| (room.min.x..=room.max.x).contains(x))
        .collect();
    xs.sort_by(|a, b| (a - toward.x).abs().total_cmp(&(b - toward.x).abs()));
    let mut nearest = None;
    let mut within = within;
    for x in xs {
        if (x - toward.x).powi(2) >= within {
            break;
        }
        let Some(y) = nearest_free_along(x, toward.y, room, grown) else {
            continue;
        };
        let at = Vec2::new(x, y);
        if at.distance_squared(toward) < within {
            within = at.distance_squared(toward);
            nearest = Some(at);
        }
    }
    nearest
}

/// The `y` nearest `toward_y` on the line through `x`, inside `room` and strictly inside
/// none of `grown`.
fn nearest_free_along(x: Scalar, toward_y: Scalar, room: &Rect, grown: &[Rect]) -> Option<Scalar> {
    let mut blocked: Vec<(Scalar, Scalar)> = grown
        .iter()
        .filter(|rect| rect.min.x < x && x < rect.max.x)
        .map(|rect| (rect.min.y, rect.max.y))
        .collect();
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Open spans: two that only touch leave the point between them free.
    let mut merged: Vec<(Scalar, Scalar)> = Vec::new();
    for (low, high) in blocked {
        match merged.last_mut() {
            Some(last) if low < last.1 => last.1 = last.1.max(high),
            _ => merged.push((low, high)),
        }
    }
    let y = toward_y.clamp(room.min.y, room.max.y);
    let Some(&(low, high)) = merged.iter().find(|(low, high)| *low < y && y < *high) else {
        return Some(y);
    };
    [low, high]
        .into_iter()
        .filter(|at| (room.min.y..=room.max.y).contains(at))
        .min_by(|a, b| (a - toward_y).abs().total_cmp(&(b - toward_y).abs()))
}

fn swapped(at: Vec2) -> Vec2 {
    Vec2::new(at.y, at.x)
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

    /// Every crossing of the grown boxes' sides tried, the slow way the walk must agree with.
    fn by_every_crossing(size: Vec2, taken: &[Rect], toward: Vec2) -> Option<Vec2> {
        let settings = settings();
        let half = size / 2.0;
        let room = Rect {
            min: Vec2::splat(settings.margin_mm) + half,
            max: PLATE - Vec2::splat(settings.margin_mm) - half,
        };
        let reach = half + Vec2::splat(settings.clearance_mm);
        let grown: Vec<Rect> = taken
            .iter()
            .map(|taken| Rect {
                min: taken.min - reach,
                max: taken.max + reach,
            })
            .collect();
        let lines = |axis: fn(Vec2) -> Scalar| -> Vec<Scalar> {
            let sides = grown
                .iter()
                .flat_map(|rect| [axis(rect.min), axis(rect.max)]);
            [axis(toward), axis(room.min), axis(room.max)]
                .into_iter()
                .chain(sides)
                .collect()
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

    #[test]
    fn walking_the_sides_finds_as_near_a_spot_as_trying_every_crossing() {
        let mut seed = 0x2545_f491_u32;
        let mut next = |span: Scalar| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            span * (seed % 10_000) as Scalar / 10_000.0
        };
        for _ in 0..300 {
            let taken: Vec<Rect> = (0..=next(12.0) as usize)
                .map(|_| {
                    let min = Vec2::new(next(100.0), next(60.0));
                    Rect {
                        min,
                        max: min + Vec2::new(1.0 + next(30.0), 1.0 + next(20.0)),
                    }
                })
                .collect();
            let (size, toward) = (
                Vec2::splat(1.0 + next(8.0)),
                Vec2::new(next(110.0), next(70.0)),
            );
            let walked = nearest_free(size, &taken, toward, PLATE, &settings());
            let tried = by_every_crossing(size, &taken, toward);
            let distance = |at: Option<Vec2>| at.map(|at| at.distance(toward));
            match (distance(walked), distance(tried)) {
                (Some(walked), Some(tried)) => assert!(
                    (walked - tried).abs() < 1e-3,
                    "{walked} mm off against {tried} mm for {taken:?} toward {toward}"
                ),
                (walked, tried) => assert_eq!(walked, tried, "{taken:?} toward {toward}"),
            }
        }
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
