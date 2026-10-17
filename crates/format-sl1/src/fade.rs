//! The firmware's `numFade` against our bottom block and ramp: it exposes layer 0 at the
//! first time and ramps to the normal one at layer `numFade - 1`; see docs/formats/sl1.md.

/// The fade count the vendor's own slicer accepts at most and at least.
const FADE_RANGE: std::ops::RangeInclusive<u32> = 3..=20;

/// The `numFade` whose ramp reaches the normal exposure on the layer ours does.
pub(crate) fn fade_layers(bottom_layers: u32, transition_layers: u16) -> u32 {
    (bottom_layers + u32::from(transition_layers) + 1).clamp(*FADE_RANGE.start(), *FADE_RANGE.end())
}

/// The one first layer and the ramp a `numFade` of `fade` exposes exactly.
pub(crate) fn bottom_and_transition(fade: u32) -> (u32, u16) {
    let ramp = fade.saturating_sub(2);
    (1, u16::try_from(ramp).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use printer_profiles::MaterialProfile;

    use super::*;

    /// Layer `index`'s exposure as the firmware computes it from `numFade`.
    fn firmware_s(fade: u32, first_s: f32, normal_s: f32, index: u32) -> f32 {
        let step = (first_s - normal_s) / (fade - 1) as f32;
        match index {
            0 => first_s,
            index if index < fade - 1 => first_s - index as f32 * step,
            _ => normal_s,
        }
    }

    #[test]
    fn a_fade_read_back_exposes_every_layer_as_the_firmware_does() {
        for fade in FADE_RANGE {
            let (bottom_layers, transition_layers) = bottom_and_transition(fade);
            let resin = MaterialProfile {
                exposure_s: 2.0,
                bottom_exposure_s: 30.0,
                bottom_layers,
                transition_layers,
                ..MaterialProfile::default()
            };
            for index in 0..fade + 3 {
                let ours = resin.exposure_of_layer_s(index);
                let theirs = firmware_s(fade, 30.0, 2.0, index);
                assert!(
                    (ours - theirs).abs() < 1e-3,
                    "numFade {fade}, layer {index}: ours {ours}, the firmware's {theirs}"
                );
            }
        }
    }

    #[test]
    fn our_ramp_is_written_as_the_fade_that_ends_on_its_layer() {
        assert_eq!(
            fade_layers(1, 8),
            10,
            "the exact case reads back as written"
        );
        assert_eq!(bottom_and_transition(fade_layers(1, 8)), (1, 8));
        assert_eq!(fade_layers(6, 3), 10);
        assert_eq!(fade_layers(0, 0), 3, "the firmware divides by numFade - 1");
        assert_eq!(fade_layers(40, 0), 20);
    }
}
