use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Africa::Cairo;

/// Calendar date in Cairo, the timezone every daily rule uses.
pub fn date(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&Cairo).date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn date_follows_cairo_midnight_not_utc() {
        // 2026-10-18 21:30 UTC is 00:30 on the 19th in Cairo (UTC+3 in summer time).
        let late = Utc.with_ymd_and_hms(2026, 10, 18, 21, 30, 0).unwrap();
        assert_eq!(date(late), NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        let early = Utc.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        assert_eq!(date(early), NaiveDate::from_ymd_opt(2026, 10, 18).unwrap());
    }
}
