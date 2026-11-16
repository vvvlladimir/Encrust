//! How a weak neck fails, and how many supports under its piece hold it.
//! See `docs/design/analysis.md`.

use std::f64::consts::PI;

use crate::stack::RiskKind;

/// Stress a neck of green resin fails at, megapascals: the most a printed tip held, one resin
/// at 0.05 mm (see docs/design/analysis.md, "Calibration").
pub const NECK_LIMIT_MPA: f64 = 13.0;
/// The share of the limit a held neck is brought to: tips cure a little under their drawn area.
const NECK_MARGIN: f64 = 0.8;
/// The most supports a single neck is given.
pub const MOST_NECK_HOLDS: usize = 16;

/// The stress a pull of `force_n` puts on a neck of `neck_mm2` bending least about `least_mm4`,
/// acting `lever_mm` off it, megapascals: tension, and bending. See `docs/design/analysis.md`.
pub(crate) fn neck_stress_mpa(force_n: f64, lever_mm: f64, neck_mm2: f64, least_mm4: f64) -> f64 {
    // The farthest fibre is read as a disc's, twice the gyration: on one disc this is exact.
    let section_modulus = (least_mm4 * neck_mm2).sqrt() / 2.0;
    force_n / neck_mm2 + force_n * lever_mm / section_modulus
}

/// The second moment of a disc of `area_mm2` about a diameter, mm⁴.
pub(crate) fn disc_mm4(area_mm2: f64) -> f64 {
    area_mm2 * area_mm2 / (4.0 * PI)
}

/// The narrowest section between a piece and the plate, and the id its lever is kept under.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Neck {
    pub(crate) area_mm2: f64,
    pub(crate) at_mm: [f64; 2],
    /// Second moments of its section about `at_mm`, `[xx, xy, yy]`, mm⁴.
    pub(crate) moments_mm4: [f64; 3],
    pub(crate) id: u32,
}

impl Neck {
    /// Necks of `area_mm2` in all side by side as one section about their joint centre, kept
    /// under the widest one's id.
    pub(crate) fn joined(necks: &[Self], area_mm2: f64) -> Self {
        let widest = necks
            .iter()
            .max_by(|a, b| a.area_mm2.total_cmp(&b.area_mm2));
        let at_mm = [0, 1].map(|axis| {
            necks
                .iter()
                .map(|neck| neck.at_mm[axis] * neck.area_mm2)
                .sum::<f64>()
                / area_mm2
        });
        // Each neck about the joint centre: its own moments, and its area off that centre.
        let mut moments_mm4 = [0.0; 3];
        for neck in necks {
            let (dx, dy) = (neck.at_mm[0] - at_mm[0], neck.at_mm[1] - at_mm[1]);
            let [xx, xy, yy] = neck.moments_mm4;
            moments_mm4[0] += xx + neck.area_mm2 * dx * dx;
            moments_mm4[1] += xy + neck.area_mm2 * dx * dy;
            moments_mm4[2] += yy + neck.area_mm2 * dy * dy;
        }
        Self {
            area_mm2,
            at_mm,
            moments_mm4,
            id: widest.map_or(u32::MAX, |neck| neck.id),
        }
    }

    /// The second moment about the axis it bends most easily round, mm⁴.
    pub(crate) fn least_mm4(&self) -> f64 {
        let [xx, xy, yy] = self.moments_mm4;
        let middle = f64::midpoint(xx, yy);
        (middle - (((xx - yy) / 2.0).powi(2) + xy * xy).sqrt()).max(0.0)
    }
}

/// What a support tip of `tip_mm2` holds in tension, newtons: the limit a neck fails at, with the
/// margin a held neck is brought to.
pub fn tip_holds_n(tip_mm2: f32) -> f64 {
    f64::from(tip_mm2) * NECK_LIMIT_MPA * NECK_MARGIN
}

/// How many supports of tip `tip_mm2` around the piece of `lever` bring its neck under the
/// limit with a margin to spare, at most [`MOST_NECK_HOLDS`]; none for another kind.
pub fn holds_for_lever(lever: &RiskKind, tip_mm2: f32) -> usize {
    let RiskKind::Lever {
        lever_mm,
        neck_mm2,
        force_n,
        gyration_mm,
        ..
    } = *lever
    else {
        return 0;
    };
    let (force_n, neck_mm2) = (f64::from(force_n), f64::from(neck_mm2));
    let swing_mm = f64::from(gyration_mm) / 2.0;
    let offset_mm = (f64::from(lever_mm) - swing_mm).max(0.0);
    let holds_it = |count: &usize| {
        let area_mm2 = neck_mm2 + *count as f64 * f64::from(tip_mm2);
        // Supports stand around the piece's centre, drawing the neck's centre to it.
        let lever_mm = offset_mm * neck_mm2 / area_mm2 + swing_mm;
        neck_stress_mpa(force_n, lever_mm, area_mm2, disc_mm4(area_mm2))
            < NECK_LIMIT_MPA * NECK_MARGIN
    };
    (1..=MOST_NECK_HOLDS)
        .find(holds_it)
        .unwrap_or(MOST_NECK_HOLDS)
}

#[cfg(test)]
mod tests;
