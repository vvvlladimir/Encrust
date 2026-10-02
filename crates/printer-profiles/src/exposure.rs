/// Penetration depth assumed of a resin that states none, millimetres: the middle of the
/// 0.05 to 0.15 mm a pigmented 405 nm resin shows; see `docs/decisions/0143`.
pub const ASSUMED_PENETRATION_DEPTH_MM: f32 = 0.10;

/// What a layer of `thickness_mm` needs, out of the `exposure_s` a resin was measured at
/// `measured_at_mm` for, along the resin's Jacobs working curve.
///
/// See `docs/decisions/0143-exposure-follows-the-working-curve.md`.
pub fn exposure_for_mm(
    exposure_s: f32,
    measured_at_mm: f32,
    thickness_mm: f32,
    penetration_depth_mm: Option<f32>,
) -> f32 {
    if measured_at_mm <= 0.0 || (thickness_mm - measured_at_mm).abs() < 1e-4 {
        return exposure_s;
    }
    let depth = penetration_depth_mm
        .filter(|depth| *depth > 0.0)
        .unwrap_or(ASSUMED_PENETRATION_DEPTH_MM);
    exposure_s * ((thickness_mm - measured_at_mm) / depth).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resin_without_a_depth_of_its_own_takes_the_assumed_one() {
        let stated = exposure_for_mm(2.0, 0.05, 0.1, Some(ASSUMED_PENETRATION_DEPTH_MM));
        assert!((exposure_for_mm(2.0, 0.05, 0.1, None) - stated).abs() < 1e-6);
    }

    #[test]
    fn halving_the_layer_takes_far_more_than_half_the_exposure() {
        // AmeraLabs: below 0.1 mm, halving the layer wants about a quarter less exposure,
        // not half. At Dp 0.1 mm the curve gives 0.78 of it; see docs/decisions/0143.
        let half = exposure_for_mm(5.2, 0.05, 0.025, None);
        assert!((half - 4.05).abs() < 0.01, "got {half}");
    }

    #[test]
    fn a_working_curve_grows_the_exposure_by_e_per_penetration_depth() {
        // Jacobs: E = Ec exp(Cd / Dp), so one Dp more of thickness is e times the dose.
        let exposure = exposure_for_mm(2.0, 0.05, 0.15, Some(0.1));
        assert!(
            (exposure - 2.0 * std::f32::consts::E).abs() < 1e-4,
            "got {exposure}"
        );
    }

    #[test]
    fn carrying_an_exposure_through_a_middle_height_is_carrying_it_straight() {
        for depth in [None, Some(0.12)] {
            let straight = exposure_for_mm(2.5, 0.05, 0.03, depth);
            let middle = exposure_for_mm(2.5, 0.05, 0.08, depth);
            let through = exposure_for_mm(middle, 0.08, 0.03, depth);
            assert!(
                (straight - through).abs() < 1e-4,
                "{straight} against {through}"
            );
        }
    }
}
