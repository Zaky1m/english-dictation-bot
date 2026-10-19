use chrono::{Datelike, Duration, NaiveDate};

pub const TIERS: [&str; 7] = [
    "bronze", "silver", "gold", "sapphire", "ruby", "emerald", "diamond",
];
pub const TOP_TIER: i16 = TIERS.len() as i16 - 1;

/// The league week starts on Saturday; this returns the Saturday on or before `day`.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    let back = (day.weekday().num_days_from_monday() + 2) % 7; // Saturday -> 0
    day - Duration::days(i64::from(back))
}

/// New tiers for one finished group. `members` are (user id, points).
/// The top `top` rise, the bottom `bottom` of the rest fall; nobody leaves the
/// edges of the ladder. Ties are broken by user id so the result is stable.
pub fn outcomes(members: &[(i64, i32)], tier: i16, top: usize, bottom: usize) -> Vec<(i64, i16)> {
    let mut ranked = members.to_vec();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let n = ranked.len();
    let promoted = top.min(n);
    let demote_from = n.saturating_sub(bottom).max(promoted);
    ranked
        .iter()
        .enumerate()
        .map(|(rank, &(user, _))| {
            let new_tier = if rank < promoted && tier < TOP_TIER {
                tier + 1
            } else if rank >= demote_from && tier > 0 {
                tier - 1
            } else {
                tier
            };
            (user, new_tier)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    #[test]
    fn week_starts_on_saturday() {
        assert_eq!(day(10, 17).weekday(), chrono::Weekday::Sat);
        assert_eq!(week_start(day(10, 17)), day(10, 17)); // Saturday itself
        assert_eq!(week_start(day(10, 18)), day(10, 17)); // Sunday
        assert_eq!(week_start(day(10, 23)), day(10, 17)); // Friday
        assert_eq!(week_start(day(10, 24)), day(10, 24)); // next Saturday
    }

    fn group(n: i64) -> Vec<(i64, i32)> {
        (1..=n).map(|i| (i, (n - i) as i32 * 10 + 10)).collect() // user 1 has the most points
    }

    fn tier_of(result: &[(i64, i16)], user: i64) -> i16 {
        result.iter().find(|(u, _)| *u == user).unwrap().1
    }

    #[test]
    fn full_group_promotes_top_seven_and_demotes_bottom_five() {
        let r = outcomes(&group(30), 2, 7, 5);
        assert_eq!(r.iter().filter(|(_, t)| *t == 3).count(), 7);
        assert_eq!(r.iter().filter(|(_, t)| *t == 1).count(), 5);
        assert_eq!(r.iter().filter(|(_, t)| *t == 2).count(), 18);
        assert_eq!(tier_of(&r, 1), 3);
        assert_eq!(tier_of(&r, 7), 3);
        assert_eq!(tier_of(&r, 8), 2);
        assert_eq!(tier_of(&r, 26), 1);
        assert_eq!(tier_of(&r, 30), 1);
    }

    #[test]
    fn nobody_leaves_the_edges() {
        let bronze = outcomes(&group(30), 0, 7, 5);
        assert!(bronze.iter().all(|(_, t)| *t >= 0));
        assert_eq!(tier_of(&bronze, 30), 0);
        assert_eq!(tier_of(&bronze, 1), 1);
        let diamond = outcomes(&group(30), TOP_TIER, 7, 5);
        assert_eq!(tier_of(&diamond, 1), TOP_TIER);
        assert_eq!(tier_of(&diamond, 30), TOP_TIER - 1);
    }

    #[test]
    fn small_groups_never_promote_and_demote_the_same_person() {
        let r = outcomes(&group(8), 3, 7, 5);
        assert_eq!(r.iter().filter(|(_, t)| *t == 4).count(), 7);
        assert_eq!(r.iter().filter(|(_, t)| *t == 2).count(), 1); // only the one left over
        let tiny = outcomes(&group(3), 3, 7, 5);
        assert!(tiny.iter().all(|(_, t)| *t == 4));
        assert!(outcomes(&[], 3, 7, 5).is_empty());
    }

    #[test]
    fn ties_are_broken_by_user_id() {
        let r = outcomes(&[(9, 50), (4, 50), (7, 50)], 2, 1, 1);
        assert_eq!(tier_of(&r, 4), 3);
        assert_eq!(tier_of(&r, 7), 2);
        assert_eq!(tier_of(&r, 9), 1);
    }
}
