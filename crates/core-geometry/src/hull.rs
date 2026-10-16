use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::{Scalar, Vec2};

/// Two points nearer than this are one point to a hull, and a bend smaller than it is
/// straight, millimetres. Well under one pixel of any panel.
const SAME_PLACE_MM: Scalar = 1e-4;

/// How far an outline may reach past the room it is fitted into and still fit,
/// millimetres: the rounding of a turn landing exactly on the wall.
const FIT_SLACK_MM: Scalar = 1e-3;

/// The convex hull of `points`, counter-clockwise, by Andrew's monotone chain. Fewer
/// than three points where they all lie on one line or in one place.
pub fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));
    sorted.dedup_by(|a, b| (*a - *b).length() < SAME_PLACE_MM);
    if sorted.len() < 3 {
        return sorted;
    }

    let mut chain: Vec<Vec2> = Vec::with_capacity(sorted.len() * 2);
    for pass in 0..2 {
        let start = chain.len();
        let walk: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 {
            Box::new(sorted.iter())
        } else {
            Box::new(sorted.iter().rev())
        };
        for &point in walk {
            while chain.len() >= start + 2
                && !turns_left(chain[chain.len() - 2], chain[chain.len() - 1], point)
            {
                chain.pop();
            }
            chain.push(point);
        }
        chain.pop();
    }
    chain
}

/// Whether `c` lies to the left of the line `a`..`b`, which is what keeps a hull convex.
fn turns_left(a: Vec2, b: Vec2, c: Vec2) -> bool {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) > SAME_PLACE_MM
}

/// The turn, radians counter-clockwise within a quarter either way, that stands `points`
/// inside a rectangle `room` across, or `None` where none does.
pub fn fitting_turn(points: &[Vec2], room: Vec2) -> Option<Scalar> {
    let outline = convex_hull(points);
    let fits = |turn: Scalar| {
        let size = turned_size(&outline, turn);
        size.x <= room.x + FIT_SLACK_MM && size.y <= room.y + FIT_SLACK_MM
    };
    if outline.is_empty() || fits(0.0) {
        return Some(0.0);
    }
    let mut events = caliper_events(&outline);
    let first = events[0];
    events.push(first + PI);
    events
        .windows(2)
        .flat_map(|span| at_the_walls(&outline, span[0], span[1], room))
        .find(|turn| fits(*turn))
        .map(|turn| if turn > FRAC_PI_2 { turn - PI } else { turn })
}

/// The width and depth of `outline` turned by `turn`.
fn turned_size(outline: &[Vec2], turn: Scalar) -> Vec2 {
    let rotation = Vec2::from_angle(turn);
    let mut low = Vec2::splat(Scalar::INFINITY);
    let mut high = Vec2::splat(Scalar::NEG_INFINITY);
    for point in outline {
        let turned = rotation.rotate(*point);
        low = low.min(turned);
        high = high.max(turned);
    }
    high - low
}

/// The turns in `[0, π)` at which a caliper of the turned box comes to lie along an edge
/// of `outline`, sorted: between two of them each caliper rests on one vertex.
fn caliper_events(outline: &[Vec2]) -> Vec<Scalar> {
    let mut events: Vec<Scalar> = (0..outline.len())
        .flat_map(|at| {
            let edge = outline[(at + 1) % outline.len()] - outline[at];
            let along = edge.x.atan2(edge.y).rem_euclid(FRAC_PI_2);
            [along, along + FRAC_PI_2]
        })
        .collect();
    events.sort_by(Scalar::total_cmp);
    events.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    events
}

/// The turns in `[from, to]` where the turned width or depth meets the room's: the ends,
/// and where one of the two sinusoids the calipers trace over the span crosses its wall.
fn at_the_walls(outline: &[Vec2], from: Scalar, to: Scalar, room: Vec2) -> Vec<Scalar> {
    let rotation = Vec2::from_angle(Scalar::midpoint(from, to));
    let span = |axis: fn(Vec2) -> Scalar| {
        let reading = |point: &&Vec2| axis(rotation.rotate(**point));
        let high = outline
            .iter()
            .max_by(|a, b| reading(a).total_cmp(&reading(b)));
        let low = outline
            .iter()
            .min_by(|a, b| reading(a).total_cmp(&reading(b)));
        high.zip(low).map_or(Vec2::ZERO, |(high, low)| *high - *low)
    };
    // Turned by t, a span d is d.x cos t - d.y sin t wide and d.x sin t + d.y cos t deep.
    let (wide, deep) = (span(|p| p.x), span(|p| p.y));
    let mut turns = vec![from, to];
    turns.extend(crossings(wide.x, -wide.y, room.x));
    turns.extend(crossings(deep.y, deep.x, room.y));
    turns.retain(|turn| (from..=to).contains(turn));
    turns
}

/// Every `t` within a turn either side of `[0, 2π)` where `a cos t + b sin t` equals `level`.
fn crossings(a: Scalar, b: Scalar, level: Scalar) -> Vec<Scalar> {
    let reach = a.hypot(b);
    if reach <= Scalar::EPSILON || level.abs() > reach {
        return Vec::new();
    }
    let (phase, off) = (b.atan2(a), (level / reach).acos());
    [-TAU, 0.0, TAU]
        .into_iter()
        .flat_map(|lap| [phase + off + lap, phase - off + lap])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(width: Scalar, depth: Scalar) -> Vec<Vec2> {
        vec![
            Vec2::ZERO,
            Vec2::new(width, 0.0),
            Vec2::new(width, depth),
            Vec2::new(0.0, depth),
        ]
    }

    #[test]
    fn a_hull_drops_the_points_inside_and_runs_counter_clockwise() {
        let mut points = rectangle(4.0, 2.0);
        points.push(Vec2::new(1.0, 1.0));
        let hull = convex_hull(&points);
        assert_eq!(hull.len(), 4, "a rectangle's four corners, got {hull:?}");
        let twice_area: Scalar = (0..hull.len())
            .map(|at| hull[at].perp_dot(hull[(at + 1) % hull.len()]))
            .sum();
        assert!(
            (twice_area - 16.0).abs() < 1e-4,
            "4 by 2, ccw, got {twice_area}"
        );
    }

    #[test]
    fn points_on_one_line_have_a_hull_of_their_two_ends() {
        let points = [Vec2::ZERO, Vec2::new(1.0, 1.0), Vec2::new(3.0, 3.0)];
        assert_eq!(convex_hull(&points), vec![Vec2::ZERO, Vec2::new(3.0, 3.0)]);
    }

    #[test]
    fn what_fits_as_it_stands_is_not_turned() {
        assert_eq!(
            fitting_turn(&rectangle(10.0, 5.0), Vec2::new(20.0, 20.0)),
            Some(0.0)
        );
    }

    #[test]
    fn a_long_part_is_turned_a_quarter_to_lie_along_the_room() {
        let turn = fitting_turn(&rectangle(5.0, 18.0), Vec2::new(20.0, 10.0))
            .expect("18 by 5 lies along a 20 by 10 room");
        let size = turned_size(&rectangle(5.0, 18.0), turn);
        assert!(
            size.x <= 20.0 + FIT_SLACK_MM && size.y <= 10.0 + FIT_SLACK_MM,
            "{size}"
        );
    }

    #[test]
    fn a_rod_longer_than_either_wall_fits_across_the_diagonal() {
        // At 45 degrees a 12.6 by 0.1 rod spans (12.6 + 0.1) / sqrt 2 = 8.98 mm each way, and
        // only within about a degree of it under 9: a window any sampling of turns can miss.
        let rod = rectangle(12.6, 0.1);
        let turn = fitting_turn(&rod, Vec2::splat(9.0)).expect("the rod fits turned");
        let size = turned_size(&rod, turn);
        assert!(size.max_element() <= 9.0 + FIT_SLACK_MM, "{size} at {turn}");
        assert!(turn.abs() > 0.1, "it had to turn, got {turn}");
    }

    #[test]
    fn a_square_wider_than_the_room_fits_no_way() {
        // A square is never narrower than its side, whichever way it is turned.
        assert_eq!(fitting_turn(&rectangle(9.5, 9.5), Vec2::splat(9.0)), None);
    }
}
