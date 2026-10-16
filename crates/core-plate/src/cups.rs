use core_geometry::{Scalar, Vec2};

/// One cut section: its pieces, each its outline and then its holes.
pub type Pieces = Vec<Vec<Vec<Vec2>>>;

/// Holes sealed from below, followed up the cut sections in print order: what each lift
/// pulls a vacuum in, the way `core-analysis` names a suction cup on the cured stack.
pub struct CupWalk {
    /// Whether the plate closes the bottom of the first section.
    on_plate: bool,
    min_mm2: Scalar,
    below: Option<Pieces>,
    sealed_below: Vec<Vec<Vec2>>,
    largest_mm2: Scalar,
}

impl CupWalk {
    pub fn new(on_plate: bool, min_mm2: Scalar) -> Self {
        Self {
            on_plate,
            min_mm2,
            below: None,
            sealed_below: Vec::new(),
            largest_mm2: 0.0,
        }
    }

    /// Takes the next section up. A hole is sealed when the point inside it stands on
    /// material, or on a hole already sealed, in the section under it. Small holes are
    /// followed too, since a cup can start narrow and widen.
    pub fn push(&mut self, pieces: Pieces) {
        let mut sealed = Vec::new();
        for hole in pieces.iter().flat_map(|piece| piece.iter().skip(1)) {
            let Some(inside) = interior_point(hole) else {
                continue;
            };
            let closed = match &self.below {
                None => self.on_plate,
                Some(below) => {
                    in_material(below, inside)
                        || self.sealed_below.iter().any(|ring| contains(ring, inside))
                }
            };
            if closed {
                let area = signed_area(hole).abs();
                if area >= self.min_mm2 {
                    self.largest_mm2 = self.largest_mm2.max(area);
                }
                sealed.push(hole.clone());
            }
        }
        self.below = Some(pieces);
        self.sealed_below = sealed;
    }

    /// The widest sealed hole met, square millimetres, or zero for none past the minimum.
    pub fn largest_mm2(&self) -> Scalar {
        self.largest_mm2
    }
}

fn in_material(pieces: &Pieces, point: Vec2) -> bool {
    pieces.iter().any(|piece| {
        piece
            .first()
            .is_some_and(|outline| contains(outline, point))
            && !piece.iter().skip(1).any(|hole| contains(hole, point))
    })
}

/// Even-odd crossing test, which reads a ring the same whichever way it is wound.
fn contains(ring: &[Vec2], point: Vec2) -> bool {
    let mut inside = false;
    for (index, a) in ring.iter().enumerate() {
        let b = ring[(index + 1) % ring.len()];
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if point.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn signed_area(ring: &[Vec2]) -> Scalar {
    let doubled: Scalar = ring
        .iter()
        .enumerate()
        .map(|(index, a)| a.perp_dot(ring[(index + 1) % ring.len()]))
        .sum();
    doubled / 2.0
}

/// A point strictly inside the ring: its centroid when that is inside, as it is for any
/// convex hole, or else the middle of the first diagonal that is.
fn interior_point(ring: &[Vec2]) -> Option<Vec2> {
    if ring.len() < 3 {
        return None;
    }
    let centroid = ring.iter().copied().sum::<Vec2>() / ring.len() as Scalar;
    std::iter::once(centroid)
        .chain((0..ring.len()).map(|at| (ring[at] + ring[(at + 2) % ring.len()]) / 2.0))
        .find(|point| contains(ring, *point))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(at: Vec2, side: Scalar) -> Vec<Vec2> {
        vec![
            at,
            at + Vec2::new(side, 0.0),
            at + Vec2::new(side, side),
            at + Vec2::new(0.0, side),
        ]
    }

    fn reversed(mut ring: Vec<Vec2>) -> Vec<Vec2> {
        ring.reverse();
        ring
    }

    /// A 30 mm square wall around a 26 mm hole.
    fn wall() -> Pieces {
        vec![vec![
            square(Vec2::ZERO, 30.0),
            reversed(square(Vec2::splat(2.0), 26.0)),
        ]]
    }

    fn floor() -> Pieces {
        vec![vec![square(Vec2::ZERO, 30.0)]]
    }

    #[test]
    fn a_wall_standing_on_a_floor_is_a_cup() {
        let mut walk = CupWalk::new(false, 25.0);
        walk.push(floor());
        walk.push(wall());
        walk.push(wall());
        assert!(
            (walk.largest_mm2() - 676.0).abs() < 1e-2,
            "the 26 by 26 inside, got {}",
            walk.largest_mm2()
        );
    }

    #[test]
    fn a_wall_open_underneath_vents_on_supports_and_seals_on_the_plate() {
        let mut lifted = CupWalk::new(false, 25.0);
        lifted.push(wall());
        lifted.push(wall());
        lifted.push(floor());
        assert_eq!(
            lifted.largest_mm2(),
            0.0,
            "air reaches it from under the model"
        );

        let mut on_plate = CupWalk::new(true, 25.0);
        on_plate.push(wall());
        on_plate.push(wall());
        assert!(on_plate.largest_mm2() > 600.0, "the plate closes it");
    }

    #[test]
    fn a_hole_under_the_minimum_is_not_named() {
        let mut walk = CupWalk::new(false, 25.0);
        walk.push(floor());
        walk.push(vec![vec![
            square(Vec2::ZERO, 30.0),
            reversed(square(Vec2::splat(10.0), 4.0)),
        ]]);
        assert_eq!(walk.largest_mm2(), 0.0, "16 mm2 pulls too little to name");
    }

    #[test]
    fn a_concave_hole_still_finds_a_point_inside_it() {
        let ell = vec![
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let point = interior_point(&ell).expect("an L has an inside");
        assert!(contains(&ell, point));
    }
}
