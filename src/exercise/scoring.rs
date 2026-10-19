use serde::Serialize;

/// Coefficients from the settings table.
#[derive(Debug, Clone)]
pub struct Params {
    pub base_constant: f64,
    pub mistake_penalty: f64,
    pub reveal_penalty: f64,
    pub min_accuracy: f64,
    pub expected_base_ms: f64,
    pub expected_per_letter_ms: f64,
    pub speed_bonus_factor: f64,
    pub combo_step: f64,
    pub combo_cap: u32,
    pub min_ms_per_letter: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Breakdown {
    pub base: i64,
    pub accuracy: f64,
    pub speed_bonus: i64,
    pub combo: u32,
    pub score: i64,
    /// Solved faster than humanly plausible; the speed bonus was removed.
    pub flagged: bool,
}

pub struct Input {
    pub letters: usize,
    pub distinct_mistakes: usize,
    pub reveals: usize,
    pub time_ms: i64,
    /// Consecutive clean sentences including this one when it is clean.
    pub combo: u32,
}

pub fn score(input: &Input, p: &Params) -> Breakdown {
    let letters = input.letters as f64;
    let base = p.base_constant + letters;
    let penalty = p.mistake_penalty * input.distinct_mistakes as f64
        + p.reveal_penalty * input.reveals as f64;
    let accuracy = (1.0 - penalty).max(p.min_accuracy);
    let flagged = (input.time_ms as f64) < p.min_ms_per_letter * letters;
    let expected_ms = p.expected_base_ms + p.expected_per_letter_ms * letters;
    let time = input.time_ms as f64;
    let speed_bonus = if !flagged && time < expected_ms {
        (base * p.speed_bonus_factor * (1.0 - time / expected_ms)).round() as i64
    } else {
        0
    };
    let combo = input.combo.min(p.combo_cap);
    let score =
        ((base * accuracy + speed_bonus as f64) * (1.0 + p.combo_step * f64::from(combo))).round();
    Breakdown {
        base: base as i64,
        accuracy,
        speed_bonus,
        combo,
        score: score as i64,
        flagged,
    }
}

/// Combo after finishing a sentence: grows on a clean one, resets otherwise.
pub fn next_combo(previous: u32, clean: bool, cap: u32) -> u32 {
    if clean { (previous + 1).min(cap) } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn params() -> Params {
        Params {
            base_constant: 10.0,
            mistake_penalty: 0.10,
            reveal_penalty: 0.25,
            min_accuracy: 0.3,
            expected_base_ms: 4000.0,
            expected_per_letter_ms: 450.0,
            speed_bonus_factor: 0.5,
            combo_step: 0.1,
            combo_cap: 5,
            min_ms_per_letter: 80.0,
        }
    }

    fn input(letters: usize, mistakes: usize, reveals: usize, time_ms: i64, combo: u32) -> Input {
        Input {
            letters,
            distinct_mistakes: mistakes,
            reveals,
            time_ms,
            combo,
        }
    }

    #[test]
    fn slow_clean_answer_gets_base_score_only() {
        // 20 letters: base 30, expected 13000 ms, so 20000 ms earns no bonus.
        let b = score(&input(20, 0, 0, 20_000, 0), &params());
        assert_eq!((b.base, b.speed_bonus, b.score), (30, 0, 30));
        assert!(!b.flagged);
    }

    #[test]
    fn speed_bonus_scales_with_time_left() {
        // Half the expected time: bonus = 30 * 0.5 * 0.5 = 7.5 -> 8.
        let b = score(&input(20, 0, 0, 6_500, 0), &params());
        assert_eq!(b.speed_bonus, 8);
        assert_eq!(b.score, 38);
    }

    #[test]
    fn mistakes_and_reveals_lower_accuracy_with_a_floor() {
        let b = score(&input(20, 2, 1, 20_000, 0), &params());
        // 1 - 0.2 - 0.25 = 0.55
        assert!((b.accuracy - 0.55).abs() < 1e-9);
        assert_eq!(b.score, 17); // 30 * 0.55 = 16.5 -> 17
        let floor = score(&input(20, 20, 0, 20_000, 0), &params());
        assert!((floor.accuracy - 0.3).abs() < 1e-9);
        assert_eq!(floor.score, 9);
    }

    #[test]
    fn combo_multiplies_and_is_capped() {
        let b = score(&input(20, 0, 0, 20_000, 3), &params());
        assert_eq!(b.score, 39); // 30 * 1.3
        let capped = score(&input(20, 0, 0, 20_000, 99), &params());
        assert_eq!((capped.combo, capped.score), (5, 45)); // 30 * 1.5
    }

    #[test]
    fn impossibly_fast_answer_is_flagged_and_gets_no_bonus() {
        // 20 letters need at least 1600 ms.
        let b = score(&input(20, 0, 0, 1_599, 0), &params());
        assert!(b.flagged);
        assert_eq!(b.speed_bonus, 0);
        let ok = score(&input(20, 0, 0, 1_600, 0), &params());
        assert!(!ok.flagged);
        assert!(ok.speed_bonus > 0);
    }

    #[test]
    fn combo_grows_on_clean_and_resets_otherwise() {
        assert_eq!(next_combo(0, true, 5), 1);
        assert_eq!(next_combo(5, true, 5), 5);
        assert_eq!(next_combo(4, false, 5), 0);
    }
}
