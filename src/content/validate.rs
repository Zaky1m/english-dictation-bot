use crate::exercise::tokens::{Token, TokenError, tokenize};

/// Word-count band per lesson number: lesson 1 is short, lesson 6 is long.
pub fn word_band(lesson: i16) -> (usize, usize) {
    match lesson {
        1 => (3, 6),
        2 => (4, 8),
        3 => (6, 10),
        4 => (7, 12),
        5 => (8, 14),
        _ => (10, 16),
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    #[error("tokenizer: {0}")]
    Token(#[from] TokenError),
    #[error("{got} words, expected {min}-{max}")]
    WordCount { got: usize, min: usize, max: usize },
    #[error("meanings do not match the words")]
    MeaningMismatch,
    #[error("empty translation")]
    NoTranslation,
}

pub struct Candidate<'a> {
    pub text: &'a str,
    pub translation: &'a str,
    /// (word as written, Arabic meaning), in sentence order.
    pub meanings: &'a [(String, String)],
}

/// Checks one generated sentence and returns its tokens. Duplicate detection
/// needs the whole bank, so the caller does that.
pub fn validate(c: &Candidate, lesson: i16) -> Result<Vec<Token>, Rejection> {
    let tokens = tokenize(c.text)?;
    let (min, max) = word_band(lesson);
    if tokens.len() < min || tokens.len() > max {
        return Err(Rejection::WordCount {
            got: tokens.len(),
            min,
            max,
        });
    }
    if c.translation.trim().is_empty() {
        return Err(Rejection::NoTranslation);
    }
    let aligned = c.meanings.len() == tokens.len()
        && c.meanings.iter().zip(&tokens).all(|((word, meaning), t)| {
            !meaning.trim().is_empty() && word.split_whitespace().eq([t.text.as_str()])
        });
    if !aligned {
        return Err(Rejection::MeaningMismatch);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meanings(words: &[&str]) -> Vec<(String, String)> {
        words
            .iter()
            .map(|w| (w.to_string(), "m".to_string()))
            .collect()
    }

    fn check(text: &str, lesson: i16, words: &[&str]) -> Result<Vec<Token>, Rejection> {
        let m = meanings(words);
        validate(
            &Candidate {
                text,
                translation: "t",
                meanings: &m,
            },
            lesson,
        )
    }

    #[test]
    fn accepts_a_good_sentence() {
        assert!(check("I am fine.", 1, &["I", "am", "fine."]).is_ok());
    }

    #[test]
    fn rejects_wrong_length_for_the_lesson() {
        assert!(matches!(
            check("Hi there.", 1, &["Hi", "there."]),
            Err(Rejection::WordCount { got: 2, .. })
        ));
        assert!(matches!(
            check("I am fine today.", 6, &["I", "am", "fine", "today."]),
            Err(Rejection::WordCount { .. })
        ));
    }

    #[test]
    fn rejects_bad_characters() {
        assert!(matches!(
            check("I have 2 cats.", 1, &["I", "have", "2", "cats."]),
            Err(Rejection::Token(TokenError::BadChar('2')))
        ));
    }

    #[test]
    fn rejects_missing_or_misaligned_meanings() {
        assert_eq!(
            check("I am fine.", 1, &["I", "am"]).unwrap_err(),
            Rejection::MeaningMismatch
        );
        assert_eq!(
            check("I am fine.", 1, &["I", "was", "fine."]).unwrap_err(),
            Rejection::MeaningMismatch
        );
        let blank = vec![
            ("I".to_string(), "a".to_string()),
            ("am".to_string(), " ".to_string()),
            ("fine.".to_string(), "c".to_string()),
        ];
        let c = Candidate {
            text: "I am fine.",
            translation: "t",
            meanings: &blank,
        };
        assert_eq!(validate(&c, 1).unwrap_err(), Rejection::MeaningMismatch);
    }

    #[test]
    fn rejects_empty_translation() {
        let m = meanings(&["I", "am", "fine."]);
        let c = Candidate {
            text: "I am fine.",
            translation: " ",
            meanings: &m,
        };
        assert_eq!(validate(&c, 1).unwrap_err(), Rejection::NoTranslation);
    }

    #[test]
    fn bands_grow_with_the_lesson() {
        let mut last = (0, 0);
        for lesson in 1..=6 {
            let band = word_band(lesson);
            assert!(band.0 >= last.0 && band.1 >= last.1);
            last = band;
        }
        assert_eq!(word_band(1), (3, 6));
        assert_eq!(word_band(6), (10, 16));
    }
}
