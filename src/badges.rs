/// Facts about a user right after a sentence was solved.
#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    pub total_sentences: i64,
    pub streak: i32,
    pub lesson_completed_now: bool,
    pub lessons_completed_total: i64,
    pub lesson_all_clean: bool,
    pub situation_completed_now: bool,
    pub letters: usize,
    pub time_ms: i64,
    pub flagged: bool,
}

/// A "fast typist" types a sentence of at least 20 letters at 150 ms per letter or faster
/// (about 80 words a minute) without tripping the anti-cheat flag.
const FAST_MIN_LETTERS: usize = 20;
const FAST_MS_PER_LETTER: i64 = 150;

/// Badge ids earned by this snapshot. Inserting an id twice is harmless, so
/// the list may include badges the user already owns.
pub fn earned(s: &Snapshot) -> Vec<&'static str> {
    let mut out = Vec::new();
    if s.lesson_completed_now && s.lessons_completed_total == 1 {
        out.push("first_lesson");
    }
    for (need, id) in [
        (10, "sentences_10"),
        (100, "sentences_100"),
        (500, "sentences_500"),
        (1000, "sentences_1000"),
    ] {
        if s.total_sentences >= need {
            out.push(id);
        }
    }
    for (need, id) in [
        (3, "streak_3"),
        (7, "streak_7"),
        (30, "streak_30"),
        (100, "streak_100"),
    ] {
        if s.streak >= need {
            out.push(id);
        }
    }
    if s.lesson_completed_now && s.lesson_all_clean {
        out.push("perfect_lesson");
    }
    if s.situation_completed_now {
        out.push("situation_completed");
    }
    if !s.flagged
        && s.letters >= FAST_MIN_LETTERS
        && s.time_ms <= FAST_MS_PER_LETTER * s.letters as i64
    {
        out.push("fast_typist");
    }
    out
}

pub const ALL: [&str; 13] = [
    "first_lesson",
    "sentences_10",
    "sentences_100",
    "sentences_500",
    "sentences_1000",
    "streak_3",
    "streak_7",
    "streak_30",
    "streak_100",
    "perfect_lesson",
    "fast_typist",
    "first_promotion",
    "situation_completed",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_unlock_every_threshold_reached() {
        let s = Snapshot {
            total_sentences: 120,
            ..Default::default()
        };
        assert_eq!(earned(&s), ["sentences_10", "sentences_100"]);
    }

    #[test]
    fn streak_badges_follow_the_current_streak() {
        let s = Snapshot {
            streak: 7,
            ..Default::default()
        };
        assert_eq!(earned(&s), ["streak_3", "streak_7"]);
    }

    #[test]
    fn first_lesson_only_when_it_is_the_first_completion() {
        let first = Snapshot {
            lesson_completed_now: true,
            lessons_completed_total: 1,
            ..Default::default()
        };
        assert!(earned(&first).contains(&"first_lesson"));
        let second = Snapshot {
            lesson_completed_now: true,
            lessons_completed_total: 2,
            ..Default::default()
        };
        assert!(!earned(&second).contains(&"first_lesson"));
        let no_completion = Snapshot {
            lessons_completed_total: 1,
            ..Default::default()
        };
        assert!(!earned(&no_completion).contains(&"first_lesson"));
    }

    #[test]
    fn perfect_lesson_needs_completion_and_all_clean() {
        let s = Snapshot {
            lesson_completed_now: true,
            lesson_all_clean: true,
            lessons_completed_total: 3,
            ..Default::default()
        };
        assert!(earned(&s).contains(&"perfect_lesson"));
        let dirty = Snapshot {
            lesson_completed_now: true,
            lessons_completed_total: 3,
            ..Default::default()
        };
        assert!(!earned(&dirty).contains(&"perfect_lesson"));
    }

    #[test]
    fn fast_typist_needs_a_long_unflagged_fast_sentence() {
        let base = Snapshot {
            letters: 20,
            time_ms: 3000,
            ..Default::default()
        };
        assert!(earned(&base).contains(&"fast_typist"));
        assert!(
            !earned(&Snapshot {
                time_ms: 3001,
                ..base.clone()
            })
            .contains(&"fast_typist")
        );
        assert!(
            !earned(&Snapshot {
                letters: 19,
                ..base.clone()
            })
            .contains(&"fast_typist")
        );
        assert!(
            !earned(&Snapshot {
                flagged: true,
                ..base
            })
            .contains(&"fast_typist")
        );
    }

    #[test]
    fn situation_badge_follows_the_flag() {
        let s = Snapshot {
            situation_completed_now: true,
            ..Default::default()
        };
        assert_eq!(earned(&s), ["situation_completed"]);
    }

    #[test]
    fn every_awardable_id_is_in_the_catalog() {
        let all = Snapshot {
            total_sentences: 1000,
            streak: 100,
            lesson_completed_now: true,
            lessons_completed_total: 1,
            lesson_all_clean: true,
            situation_completed_now: true,
            letters: 30,
            time_ms: 100,
            flagged: false,
        };
        for id in earned(&all) {
            assert!(ALL.contains(&id), "{id}");
        }
    }
}
