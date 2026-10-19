use super::tokens::Token;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct WordResult {
    /// Zero-based letter positions that are wrong.
    pub wrong: Vec<usize>,
    pub complete: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    pub words: Vec<WordResult>,
    pub solved: bool,
}

/// A wrong letter seen for the first time: (word index, position, typed letter).
pub type Mistake = (usize, usize, char);

/// Lowercases and keeps only `a-z`, the same filter the client applies.
pub fn sanitize(typed: &str) -> String {
    typed
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Compares typed words with the sentence. Missing words and letters count as
/// incomplete, not wrong; extra letters past the end count as wrong.
pub fn check(tokens: &[Token], typed: &[String]) -> (CheckResult, Vec<Mistake>) {
    let mut mistakes = Vec::new();
    let words: Vec<WordResult> = tokens
        .iter()
        .enumerate()
        .map(|(w, token)| {
            let typed = typed.get(w).map(|t| sanitize(t)).unwrap_or_default();
            let expected: Vec<char> = token.letters.chars().collect();
            let wrong: Vec<usize> = typed
                .chars()
                .enumerate()
                .filter(|&(i, c)| expected.get(i) != Some(&c))
                .map(|(i, c)| {
                    mistakes.push((w, i, c));
                    i
                })
                .collect();
            WordResult {
                complete: wrong.is_empty() && typed == token.letters,
                wrong,
            }
        })
        .collect();
    let solved = words.iter().all(|w| w.complete);
    (CheckResult { words, solved }, mistakes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::tokens::tokenize;

    fn run(sentence: &str, typed: &[&str]) -> (CheckResult, Vec<Mistake>) {
        let typed: Vec<String> = typed.iter().map(|s| s.to_string()).collect();
        check(&tokenize(sentence).unwrap(), &typed)
    }

    #[test]
    fn correct_answer_solves() {
        let (r, m) = run("I'm sorry.", &["im", "sorry"]);
        assert!(r.solved && m.is_empty());
        assert!(r.words.iter().all(|w| w.complete && w.wrong.is_empty()));
    }

    #[test]
    fn comparison_ignores_case() {
        assert!(run("Hello there", &["HELLO", "ThErE"]).0.solved);
    }

    #[test]
    fn marks_wrong_positions_and_reports_mistakes() {
        let (r, m) = run("coffee please", &["cofee", ""]);
        assert_eq!(r.words[0].wrong, [3]);
        assert!(!r.words[0].complete && !r.solved);
        assert_eq!(m, [(0, 3, 'e')]);
    }

    #[test]
    fn partial_input_is_incomplete_but_not_wrong() {
        let (r, m) = run("coffee", &["cof"]);
        assert!(r.words[0].wrong.is_empty() && !r.words[0].complete);
        assert!(m.is_empty());
    }

    #[test]
    fn extra_letters_are_wrong() {
        let (r, m) = run("cup", &["cupp"]);
        assert_eq!(r.words[0].wrong, [3]);
        assert_eq!(m, [(0, 3, 'p')]);
        assert!(!r.solved);
    }

    #[test]
    fn missing_and_empty_words_do_not_solve() {
        assert!(!run("good morning", &["good"]).0.solved);
        assert!(!run("good morning", &["good", ""]).0.solved);
    }

    #[test]
    fn extra_words_are_ignored() {
        assert!(run("hi", &["hi", "extra"]).0.solved);
    }

    #[test]
    fn typed_noise_is_filtered() {
        assert!(run("it's me", &["it's", "m3e"]).0.solved);
        assert_eq!(sanitize("ab-1 \u{0627}C"), "abc");
    }
}
