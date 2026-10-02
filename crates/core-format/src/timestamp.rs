use std::time::{SystemTime, UNIX_EPOCH};

/// The current UTC time as `YYYY-MM-DD hh:mm:ss`, the stamp sliced files carry.
///
/// A file whose clock cannot be read gets the epoch rather than an error: the field is
/// informational and no printer refuses a file over it.
pub fn now_utc() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    format_utc(seconds)
}

/// Minutes since the Unix epoch, which is the unit several containers stamp in.
pub fn minutes_since_epoch() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() / 60) as u32
}

fn format_utc(seconds: u64) -> String {
    let days = (seconds / 86_400).cast_signed();
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (rest / 3600, (rest / 60) % 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// Days since 1970-01-01 as a calendar date.
///
/// Howard Hinnant's `civil_from_days`, which shifts the era so that a leap day lands at
/// the end of the year: <https://howardhinnant.github.io/date_algorithms.html>
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;

    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_formats_as_the_first_of_january_1970() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00");
    }

    #[test]
    fn a_known_instant_formats_correctly() {
        // 2024-02-29T13:45:01Z, a leap day, which is what the era shift exists for.
        assert_eq!(format_utc(1_709_214_301), "2024-02-29 13:45:01");
    }

    #[test]
    fn the_string_fits_the_twenty_four_byte_field() {
        assert!(now_utc().len() <= 24);
    }
}
