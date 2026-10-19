use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreakState {
    pub current: i32,
    pub best: i32,
    pub last_day: Option<NaiveDate>,
    pub freezes_left: i32,
    pub freezes_reset_month: Option<NaiveDate>,
}

pub fn month_start(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

/// Pro users get their monthly freezes at the first activity of a month.
fn refill(state: &mut StreakState, today: NaiveDate, pro: bool, per_month: i32) {
    let month = month_start(today);
    if state.freezes_reset_month != Some(month) {
        state.freezes_left = if pro { per_month } else { 0 };
        state.freezes_reset_month = Some(month);
    }
}

/// Called when the user reaches the daily goal on `today`. Missed days in
/// between are covered by freezes, Pro only; otherwise the streak restarts at 1.
pub fn record_goal_day(
    mut state: StreakState,
    today: NaiveDate,
    pro: bool,
    per_month: i32,
) -> StreakState {
    if state.last_day == Some(today) {
        return state;
    }
    refill(&mut state, today, pro, per_month);
    let missed = state.last_day.map(|d| (today - d).num_days() - 1);
    state.current = match missed {
        None => 1,
        Some(m) if m <= 0 => state.current + 1,
        Some(m) if pro && i64::from(state.freezes_left) >= m => {
            state.freezes_left -= m as i32;
            state.current + 1
        }
        Some(_) => 1,
    };
    state.best = state.best.max(state.current);
    state.last_day = Some(today);
    state
}

/// The streak to display today: still alive if yesterday counted, or if
/// freezes would cover the days missed since.
pub fn effective(state: &StreakState, today: NaiveDate, pro: bool) -> i32 {
    let Some(last) = state.last_day else { return 0 };
    let missed = (today - last).num_days() - 1;
    if missed <= 0 || (pro && i64::from(state.freezes_left) >= missed) {
        state.current
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    fn after(state: &StreakState, today: NaiveDate, pro: bool) -> StreakState {
        record_goal_day(state.clone(), today, pro, 2)
    }

    #[test]
    fn first_goal_day_starts_a_streak() {
        let s = after(&StreakState::default(), day(10, 18), false);
        assert_eq!((s.current, s.best, s.last_day), (1, 1, Some(day(10, 18))));
    }

    #[test]
    fn consecutive_days_extend_and_same_day_is_idempotent() {
        let s = after(&StreakState::default(), day(10, 18), false);
        let s = after(&s, day(10, 19), false);
        assert_eq!(s.current, 2);
        assert_eq!(after(&s, day(10, 19), false), s);
    }

    #[test]
    fn a_missed_day_resets_a_free_streak_but_keeps_the_best() {
        let mut s = after(&StreakState::default(), day(10, 18), false);
        s = after(&s, day(10, 19), false);
        s = after(&s, day(10, 21), false);
        assert_eq!((s.current, s.best), (1, 2));
    }

    #[test]
    fn pro_freeze_bridges_a_missed_day_and_is_consumed() {
        let mut s = after(&StreakState::default(), day(10, 18), true);
        assert_eq!(s.freezes_left, 2);
        s = after(&s, day(10, 20), true);
        assert_eq!((s.current, s.freezes_left), (2, 1));
    }

    #[test]
    fn pro_gap_longer_than_freezes_resets() {
        let mut s = after(&StreakState::default(), day(10, 18), true);
        s = after(&s, day(10, 22), true); // 3 missed days, 2 freezes
        assert_eq!((s.current, s.freezes_left), (1, 2));
    }

    #[test]
    fn freezes_refill_in_a_new_month_for_pro_only() {
        let mut s = after(&StreakState::default(), day(10, 30), true);
        s = after(&s, day(11, 2), true); // missed Oct 31 and Nov 1, new month refills first
        assert_eq!((s.current, s.freezes_left), (2, 0));
        let free = after(
            &StreakState {
                freezes_left: 5,
                ..Default::default()
            },
            day(10, 18),
            false,
        );
        assert_eq!(free.freezes_left, 0);
    }

    #[test]
    fn effective_streak_expires_without_freezes() {
        let s = after(
            &after(&StreakState::default(), day(10, 18), false),
            day(10, 19),
            false,
        );
        assert_eq!(effective(&s, day(10, 19), false), 2); // done today
        assert_eq!(effective(&s, day(10, 20), false), 2); // yesterday counted, today still open
        assert_eq!(effective(&s, day(10, 21), false), 0); // missed a full day
        assert_eq!(effective(&StreakState::default(), day(10, 21), false), 0);
    }

    #[test]
    fn effective_streak_survives_a_covered_gap_for_pro() {
        let s = after(&StreakState::default(), day(10, 18), true);
        assert_eq!(effective(&s, day(10, 20), true), 1); // one missed day, freezes available
        assert_eq!(effective(&s, day(10, 22), true), 0); // three missed days
    }
}
