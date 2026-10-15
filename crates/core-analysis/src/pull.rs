use std::f64::consts::PI;

/// Pull per square millimetre of cured area whatever its shape, N/mm^2: set so a 100 cm^2
/// disc pulls about 100 N, the order users report. See `docs/design/analysis.md`.
pub const PULL_N_PER_MM2: f64 = 1.95e-3;
/// Pull per square millimetre and millimetre of reach, N/mm^3: the suction of resin drawn
/// in from the edge, against the area's own pull as measured by Pan et al. (2017).
pub const PULL_N_PER_MM3: f64 = 0.146 * PULL_N_PER_MM2;
/// Pull on the film past which a layer tears or lifts the model off the plate, newtons:
/// a solid disc some 73 mm across. A warning level, like the neck's.
pub const PEEL_LIMIT_N: f64 = 30.0;

/// What a piece of `area_mm2`, with `polar_mm4` its second moment about its own centre,
/// pulls on the film as the plate lifts it, newtons.
pub fn pull_n(area_mm2: f64, polar_mm4: f64) -> f64 {
    if area_mm2 <= 0.0 {
        return 0.0;
    }
    area_mm2 * (PULL_N_PER_MM2 + PULL_N_PER_MM3 * reach_mm(area_mm2, polar_mm4))
}

/// How far resin is drawn in from the piece's edge, millimetres: its area over its
/// perimeter for a disc, within a fifth of it for a square or a strip.
fn reach_mm(area_mm2: f64, polar_mm4: f64) -> f64 {
    if polar_mm4 <= 0.0 {
        return 0.0;
    }
    (area_mm2.powi(3) / (8.0 * PI * PI * polar_mm4)).sqrt()
}

/// What a solid disc `diameter_mm` across pulls, newtons.
pub fn disc_pull_n(diameter_mm: f64) -> f64 {
    let radius = diameter_mm / 2.0;
    let area = PI * radius * radius;
    pull_n(area, area * radius * radius / 2.0)
}

/// The diameter of the solid disc that pulls `pull_n`, millimetres: how a pull is worth
/// stating to someone holding a print.
pub fn equivalent_disc_mm(pull_n: f64) -> f64 {
    if pull_n <= 0.0 {
        return 0.0;
    }
    let mut high = 1.0;
    while disc_pull_n(high) < pull_n {
        high *= 2.0;
    }
    let mut low = 0.0;
    for _ in 0..60 {
        let mid = f64::midpoint(low, high);
        if disc_pull_n(mid) < pull_n {
            low = mid;
        } else {
            high = mid;
        }
    }
    f64::midpoint(low, high)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `width` by `length` rectangle's area and polar moment about its centre.
    fn rectangle(width: f64, length: f64) -> (f64, f64) {
        let area = width * length;
        (area, area * (width * width + length * length) / 12.0)
    }

    #[test]
    fn a_disc_reaches_in_by_its_area_over_its_perimeter() {
        let radius: f64 = 20.0;
        let area = PI * radius * radius;
        let reach = reach_mm(area, area * radius * radius / 2.0);
        assert!(
            (reach - radius / 2.0).abs() < 1e-9,
            "a disc's area over its perimeter is R / 2 = {}, got {reach}",
            radius / 2.0
        );
    }

    #[test]
    fn a_square_and_a_strip_reach_in_within_a_fifth_of_their_area_over_perimeter() {
        for (width, length) in [(10.0, 10.0), (2.0, 80.0)] {
            let (area, polar) = rectangle(width, length);
            let over_perimeter = area / (2.0 * (width + length));
            let reach = reach_mm(area, polar);
            assert!(
                (reach / over_perimeter - 1.0).abs() < 0.25,
                "{width} by {length}: A / L = {over_perimeter}, got {reach}"
            );
        }
    }

    #[test]
    fn a_hundred_square_centimetre_disc_pulls_about_a_hundred_newtons() {
        let diameter = 2.0 * (10_000.0 / PI).sqrt();
        let pull = disc_pull_n(diameter);
        assert!(
            (pull - 100.0).abs() < 1.0,
            "the calibration point, got {pull}"
        );
    }

    #[test]
    fn shape_moves_the_pull_as_pan_measured_at_one_area() {
        // Pan et al. (2017): 314 mm2 cylinder 6.16 N, star 4.9 N at A / L of 5 and 2.58.
        let disc = disc_pull_n(20.0);
        let star = 314.16 * (PULL_N_PER_MM2 + PULL_N_PER_MM3 * 2.58);
        let measured = 4.9 / 6.16;
        assert!(
            (star / disc - measured).abs() < 0.02,
            "star over disc should be {measured}, got {}",
            star / disc
        );
    }

    #[test]
    fn a_strip_pulls_by_its_area_not_next_to_nothing() {
        let (area, polar) = rectangle(1.0, 40.0);
        let strip = pull_n(area, polar);
        let (area, polar) = rectangle(40f64.sqrt(), 40f64.sqrt());
        let square = pull_n(area, polar);
        assert!(
            strip < square && strip > square / 2.0,
            "a strip pulls less than a square of its area but the same order: {strip} {square}"
        );
    }

    #[test]
    fn the_equivalent_disc_pulls_as_hard() {
        for pull in [0.5, 10.0, PEEL_LIMIT_N, 250.0] {
            let diameter = equivalent_disc_mm(pull);
            assert!(
                (disc_pull_n(diameter) - pull).abs() < 1e-6 * pull,
                "{pull} N back from {diameter} mm"
            );
        }
        assert_eq!(equivalent_disc_mm(0.0), 0.0);
    }
}
