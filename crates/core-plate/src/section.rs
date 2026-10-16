use core_analysis::pull_n;
use core_geometry::{Scalar, Vec2};

/// What one cut through the model pulls on the film as the plate lifts it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Section {
    pub area_mm2: Scalar,
    /// What the pieces pull, summed, newtons: the reading `core-analysis` takes of a cured
    /// layer. See `docs/design/analysis.md`.
    pub pull_n: Scalar,
}

impl Section {
    /// The section the pieces of one plane make, each its outline and then its holes.
    pub fn of(pieces: &[Vec<Vec<Vec2>>]) -> Self {
        let (area, pull) = pieces
            .iter()
            .map(|rings| piece(rings))
            .fold((0.0, 0.0), |(area, pull), (a, p)| (area + a, pull + p));
        Self {
            area_mm2: area as Scalar,
            pull_n: pull as Scalar,
        }
    }
}

/// The area and the pull of one piece. Its rings are wound opposite ways, so their signed
/// moments add up to the piece with its holes taken out.
fn piece(rings: &[Vec<Vec2>]) -> (f64, f64) {
    // Moments taken about a point of the piece, so a part far out on the plate loses no
    // precision to its distance from the origin.
    let Some(origin) = rings.first().and_then(|ring| ring.first()).copied() else {
        return (0.0, 0.0);
    };
    let mut sums = [0.0f64; 4];
    for ring in rings {
        for (index, from) in ring.iter().enumerate() {
            let to = ring[(index + 1) % ring.len()];
            let (x0, y0) = (f64::from(from.x - origin.x), f64::from(from.y - origin.y));
            let (x1, y1) = (f64::from(to.x - origin.x), f64::from(to.y - origin.y));
            let cross = x0 * y1 - x1 * y0;
            sums[0] += cross / 2.0;
            sums[1] += (x0 + x1) * cross / 6.0;
            sums[2] += (y0 + y1) * cross / 6.0;
            sums[3] += (x0 * x0 + x0 * x1 + x1 * x1 + y0 * y0 + y0 * y1 + y1 * y1) * cross / 12.0;
        }
    }
    let sign = sums[0].signum();
    let [area, x, y, polar] = sums.map(|sum| sum * sign);
    if area <= 0.0 {
        return (0.0, 0.0);
    }
    let polar = polar - (x * x + y * y) / area;
    (area, pull_n(area, polar))
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

    fn disc(radius: Scalar, sides: usize) -> Vec<Vec2> {
        (0..sides)
            .map(|index| {
                let angle = std::f32::consts::TAU * index as Scalar / sides as Scalar;
                Vec2::new(radius * angle.cos(), radius * angle.sin()) + Vec2::splat(100.0)
            })
            .collect()
    }

    #[test]
    fn a_cut_disc_pulls_as_the_cured_disc_does() {
        let section = Section::of(&[vec![disc(10.0, 720)]]);
        let cured = core_analysis::disc_pull_n(20.0) as Scalar;
        assert!(
            (section.pull_n - cured).abs() / cured < 1e-3,
            "the search and the proof read one law: {cured} N, got {}",
            section.pull_n
        );
    }

    #[test]
    fn a_thin_strip_pulls_less_than_a_square_of_the_same_area() {
        let strip = vec![
            Vec2::ZERO,
            Vec2::new(40.0, 0.0),
            Vec2::new(40.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let thin = Section::of(&[vec![strip]]);
        let square = Section::of(&[vec![square(Vec2::ZERO, 40f32.sqrt())]]);
        assert!(
            (thin.area_mm2 - square.area_mm2).abs() < 1e-3,
            "both are 40 mm2"
        );
        assert!(
            thin.pull_n < square.pull_n,
            "resin reaches a strip's middle sooner, got {} against {}",
            thin.pull_n,
            square.pull_n
        );
    }

    #[test]
    fn two_pieces_pull_apart_and_less_than_one_of_both_their_areas() {
        let apart = Section::of(&[
            vec![square(Vec2::ZERO, 10.0)],
            vec![square(Vec2::new(50.0, 0.0), 10.0)],
        ]);
        let single = Section::of(&[vec![square(Vec2::ZERO, 10.0)]]);
        let one = Section::of(&[vec![square(Vec2::ZERO, 200f32.sqrt())]]);
        assert!((apart.area_mm2 - 200.0).abs() < 1e-3);
        assert!((apart.pull_n - 2.0 * single.pull_n).abs() / single.pull_n < 1e-3);
        assert!(
            apart.pull_n < one.pull_n,
            "resin reaches each half from its own edge"
        );
    }

    #[test]
    fn a_hole_takes_its_area_and_its_pull_out_of_the_piece() {
        let mut hole = square(Vec2::splat(4.0), 2.0);
        hole.reverse();
        let ring = Section::of(&[vec![square(Vec2::ZERO, 10.0), hole]]);
        let solid = Section::of(&[vec![square(Vec2::ZERO, 10.0)]]);
        assert!((ring.area_mm2 - 96.0).abs() < 1e-3, "100 - 4");
        assert!(ring.pull_n < solid.pull_n);
    }
}
