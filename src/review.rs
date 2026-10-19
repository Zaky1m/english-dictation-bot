use chrono::{Duration, NaiveDate};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Remove from the queue: reviewed cleanly.
    Graduate,
    /// Keep in the queue at this stage, due on this date.
    Schedule { stage: usize, due_on: NaiveDate },
}

/// Decides what happens to a sentence after an attempt.
///
/// `stage` is the current queue stage, or `None` for a sentence that was not
/// queued. A clean attempt on a queued sentence graduates it; a clean attempt
/// on a normal sentence changes nothing. A mistake or reveal queues it at the
/// first interval, or moves a queued one to the next (the last interval repeats).
pub fn after_attempt(
    stage: Option<usize>,
    clean: bool,
    today: NaiveDate,
    intervals_days: &[i64],
) -> Option<Action> {
    let last = intervals_days.len().checked_sub(1)?;
    match (stage, clean) {
        (Some(_), true) => Some(Action::Graduate),
        (None, true) => None,
        (stage, false) => {
            let next = stage.map_or(0, |s| (s + 1).min(last));
            Some(Action::Schedule {
                stage: next,
                due_on: today + Duration::days(intervals_days[next]),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERVALS: [i64; 3] = [1, 3, 7];

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    #[test]
    fn first_mistake_queues_for_tomorrow() {
        assert_eq!(
            after_attempt(None, false, day(18), &INTERVALS),
            Some(Action::Schedule {
                stage: 0,
                due_on: day(19)
            })
        );
    }

    #[test]
    fn repeated_mistakes_walk_through_the_intervals() {
        assert_eq!(
            after_attempt(Some(0), false, day(19), &INTERVALS),
            Some(Action::Schedule {
                stage: 1,
                due_on: day(22)
            })
        );
        assert_eq!(
            after_attempt(Some(1), false, day(22), &INTERVALS),
            Some(Action::Schedule {
                stage: 2,
                due_on: day(29)
            })
        );
        // The last interval repeats.
        assert_eq!(
            after_attempt(Some(2), false, day(29), &INTERVALS),
            Some(Action::Schedule {
                stage: 2,
                due_on: NaiveDate::from_ymd_opt(2026, 11, 5).unwrap()
            })
        );
    }

    #[test]
    fn clean_review_graduates() {
        assert_eq!(
            after_attempt(Some(1), true, day(22), &INTERVALS),
            Some(Action::Graduate)
        );
    }

    #[test]
    fn clean_first_attempt_changes_nothing() {
        assert_eq!(after_attempt(None, true, day(18), &INTERVALS), None);
    }

    #[test]
    fn no_intervals_means_no_review() {
        assert_eq!(after_attempt(None, false, day(18), &[]), None);
    }
}
