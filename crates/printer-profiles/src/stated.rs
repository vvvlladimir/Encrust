//! A resin setting as a file another program wrote states it, kept only where a printer
//! could take it: a tilting vat's file carries placeholders, and some programs write zeros.

/// A lift or retract distance in millimetres, or `None` for a placeholder.
pub fn stated_distance_mm(value: f32) -> Option<f32> {
    // A tilting vat's file states the layer height or zero; the shortest real peel is a few
    // millimetres, so half of one is the floor.
    (0.5..=100.0).contains(&value).then_some(value)
}

/// A lift or retract speed in millimetres a minute, or `None` for a placeholder.
pub fn stated_speed_mm_min(value: f32) -> Option<f32> {
    // A tilting vat's file states 0.05; real peels run at tens to a few hundred mm/min.
    (1.0..=1000.0).contains(&value).then_some(value)
}

/// A wait or a light-off delay in seconds, or `None` where it cannot be one.
pub fn stated_wait_s(value: f32) -> Option<f32> {
    (0.0..=600.0).contains(&value).then_some(value)
}

/// A UV power as the 0 to 255 a resin holds, or `None` for a value no panel takes.
pub fn stated_pwm(value: u16) -> Option<u8> {
    // Zero is a field the file never set, not a resin printing with the light off.
    u8::try_from(value).ok().filter(|pwm| *pwm > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tilting_vats_placeholders_are_not_taken_for_motion() {
        // The Saturn 4 Ultra's file states its 0.035 mm layer and 0.05 mm/min.
        assert_eq!(stated_distance_mm(0.035), None);
        assert_eq!(stated_speed_mm_min(0.05), None);
        assert_eq!(stated_distance_mm(0.0), None);
        assert_eq!(stated_distance_mm(5.3), Some(5.3));
        assert_eq!(stated_speed_mm_min(61.0), Some(61.0));
    }

    #[test]
    fn a_speed_in_the_wrong_unit_is_dropped_rather_than_kept() {
        assert_eq!(stated_speed_mm_min(4260.0 * 60.0), None);
    }

    #[test]
    fn a_wait_is_any_non_negative_time_short_of_ten_minutes() {
        assert_eq!(stated_wait_s(0.0), Some(0.0));
        assert_eq!(stated_wait_s(-1.0), None);
        assert_eq!(stated_wait_s(f32::NAN), None);
    }

    #[test]
    fn a_power_is_one_to_full() {
        assert_eq!(stated_pwm(217), Some(217));
        assert_eq!(stated_pwm(0), None);
        assert_eq!(stated_pwm(256), None);
    }
}
