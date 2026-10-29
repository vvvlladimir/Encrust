//! A lift as both Photon Workshop containers state it: always two stages, the first lift
//! paired with the second retract, the order the bed moves in. See `docs/formats/anycubic.md`.

use core_format::LayerPeel;

/// Each stage's height, speed up and speed down, mm and mm/s, as a two-stage resin states
/// them.
pub(crate) fn paired_mm_s(peel: &LayerPeel) -> [[f32; 3]; 2] {
    [
        [
            peel.lift_mm,
            per_s(peel.lift_speed_mm_min),
            per_s(peel.second_retract_speed_mm_min),
        ],
        [
            peel.second_lift_mm,
            per_s(peel.second_lift_speed_mm_min),
            per_s(peel.retract_speed_mm_min),
        ],
    ]
}

/// The one stage of a resin without a second, its height, speed up and speed down.
pub(crate) fn single_mm_s(peel: &LayerPeel) -> [f32; 3] {
    [
        peel.lift_mm,
        per_s(peel.lift_speed_mm_min),
        per_s(peel.retract_speed_mm_min),
    ]
}

fn per_s(mm_min: f32) -> f32 {
    mm_min / 60.0
}
