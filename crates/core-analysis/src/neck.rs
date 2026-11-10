//! How a weak neck fails, and how many supports under its piece hold it.
//! See `docs/design/analysis.md`.

use std::f64::consts::PI;

use crate::stack::RiskKind;

/// Stress a neck of green resin is taken to fail at, megapascals. Not measured: a warning
/// level.
pub const NECK_LIMIT_MPA: f64 = 5.0;
/// The share of the limit a held neck is brought to: tips cure a little under their drawn area.
const NECK_MARGIN: f64 = 0.8;
/// The most supports a single neck is given.
pub const MOST_NECK_HOLDS: usize = 16;

/// The stress a pull of `force_n` puts on a round neck of `neck_mm2`, acting `lever_mm` off it,
/// megapascals: tension, and bending. See `docs/design/analysis.md`.
pub(crate) fn neck_stress_mpa(force_n: f64, lever_mm: f64, neck_mm2: f64) -> f64 {
    let radius_mm = (neck_mm2 / PI).sqrt();
    let section_modulus = PI * radius_mm.powi(3) / 4.0;
    force_n / neck_mm2 + force_n * lever_mm / section_modulus
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
        neck_stress_mpa(force_n, lever_mm, area_mm2) < NECK_LIMIT_MPA * NECK_MARGIN
    };
    (1..=MOST_NECK_HOLDS)
        .find(holds_it)
        .unwrap_or(MOST_NECK_HOLDS)
}

#[cfg(test)]
mod tests;
