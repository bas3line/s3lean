//! `x-amz-date` from a `SystemTime`, without a date crate: seconds since the
//! epoch to a civil date is thirty lines, and it is the only calendar
//! arithmetic signing needs.

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns `YYYYMMDDTHHMMSSZ` and its `YYYYMMDD` prefix, in UTC. A time before
/// the epoch is treated as the epoch, since no signature can be valid then.
pub(crate) fn amz_date(at: SystemTime) -> (String, String) {
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let date = format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        remainder / 3600,
        (remainder % 3600) / 60,
        remainder % 60
    );
    let short = date[..8].to_owned();
    (date, short)
}

/// Howard Hinnant's algorithm: days since 1970-01-01 to a proleptic Gregorian
/// date, exact for every day the `i64` can hold.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn formats_the_dates_the_signing_examples_use() {
        assert_eq!(amz_date(at(1_369_353_600)).0, "20130524T000000Z");
        assert_eq!(amz_date(at(1_369_353_600)).1, "20130524");
    }

    #[test]
    fn handles_the_epoch_leap_days_and_century_rules() {
        assert_eq!(amz_date(at(0)).0, "19700101T000000Z");
        assert_eq!(amz_date(at(951_782_400)).0, "20000229T000000Z");
        assert_eq!(amz_date(at(951_868_800)).0, "20000301T000000Z");
        assert_eq!(amz_date(at(4_107_542_399)).0, "21000228T235959Z");
        assert_eq!(amz_date(at(1_758_412_800 + 3_661)).0, "20250921T010101Z");
    }
}
