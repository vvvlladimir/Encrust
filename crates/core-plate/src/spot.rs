use core_geometry::{Scalar, Vec2};

use crate::arrange::ArrangeSettings;

/// A box across the plate, millimetres: what a model, its supports or its raft cover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

/// The centre nearest `toward` for a part `size_mm` across on a plate `plate_mm` across,
/// `settings.clearance_mm` clear of every one of `taken` and `settings.margin_mm` in from the
/// edge, or `None` where no room is left on the plate.
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
    // The nearest free point is `toward` itself or lies on a side of a box grown by the part,
    // so only those sides are walked, nearest first: exact, with no grid.
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
mod tests;
