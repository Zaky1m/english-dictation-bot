use serde::{Deserialize, Serialize};

/// Characters that are pre-printed in the input boxes and never typed.
pub const FIXED_CHARS: [char; 6] = ['\'', '?', '.', ',', '!', '-'];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    /// The word exactly as written in the sentence.
    pub text: String,
    /// One `_` per letter to type, fixed characters kept in place.
    pub pattern: String,
    /// Lowercase letters the learner types.
    pub letters: String,
    /// Normalized key used for word audio and meanings.
    pub word: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("sentence is empty")]
    Empty,
    #[error("character {0:?} is not allowed")]
    BadChar(char),
    #[error("word {0:?} has no letters")]
    NoLetters(String),
}

pub fn tokenize(sentence: &str) -> Result<Vec<Token>, TokenError> {
    let tokens = sentence
        .split_whitespace()
        .map(token)
        .collect::<Result<Vec<_>, _>>()?;
    if tokens.is_empty() {
        return Err(TokenError::Empty);
    }
    Ok(tokens)
}

fn token(text: &str) -> Result<Token, TokenError> {
    let mut pattern = String::new();
    let mut letters = String::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            pattern.push('_');
            letters.push(c.to_ascii_lowercase());
            word.push(c.to_ascii_lowercase());
        } else if FIXED_CHARS.contains(&c) {
            pattern.push(c);
            // Sentence punctuation is not part of the word; apostrophe and hyphen are.
            if c == '\'' || c == '-' {
                word.push(c);
            }
        } else {
            return Err(TokenError::BadChar(c));
        }
    }
    if letters.is_empty() {
        return Err(TokenError::NoLetters(text.to_owned()));
    }
    let word = word.trim_matches(['\'', '-']).to_owned();
    Ok(Token {
        text: text.to_owned(),
        pattern,
        letters,
        word,
    })
}

/// Lowercased sentence without punctuation, used to detect duplicates.
pub fn normalize(sentence: &str) -> String {
    sentence
        .chars()
        .filter(|c| c.is_ascii_alphabetic() || c.is_whitespace() || *c == '\'')
        .map(|c| c.to_ascii_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> Token {
        tokenize(text).unwrap().remove(0)
    }

    #[test]
    fn plain_word() {
        let t = one("Coffee");
        assert_eq!(t.pattern, "______");
        assert_eq!(t.letters, "coffee");
        assert_eq!(t.word, "coffee");
    }

    #[test]
    fn apostrophe_is_fixed_and_not_typed() {
        let t = one("I'm");
        assert_eq!(t.pattern, "_'_");
        assert_eq!(t.letters, "im");
        assert_eq!(t.word, "i'm");
    }

    #[test]
    fn trailing_punctuation_is_fixed_but_not_in_word() {
        let t = one("please.");
        assert_eq!(t.pattern, "______.");
        assert_eq!(t.letters, "please");
        assert_eq!(t.word, "please");
        assert_eq!(one("what?!").pattern, "____?!");
    }

    #[test]
    fn hyphen_stays_inside_word() {
        let t = one("well-known");
        assert_eq!(t.pattern, "____-_____");
        assert_eq!(t.letters, "wellknown");
        assert_eq!(t.word, "well-known");
    }

    #[test]
    fn splits_on_any_whitespace() {
        let t = tokenize("Could  you\thelp me?").unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t[3].pattern, "__?");
    }

    #[test]
    fn rejects_disallowed_characters() {
        assert_eq!(tokenize("Hello 1"), Err(TokenError::BadChar('1')));
        assert_eq!(tokenize("caf\u{e9}"), Err(TokenError::BadChar('\u{e9}')));
        assert_eq!(tokenize("a/b"), Err(TokenError::BadChar('/')));
    }

    #[test]
    fn rejects_empty_and_letterless_words() {
        assert_eq!(tokenize("   "), Err(TokenError::Empty));
        assert!(matches!(tokenize("Hi ..."), Err(TokenError::NoLetters(_))));
    }

    #[test]
    fn normalize_ignores_case_and_punctuation() {
        assert_eq!(normalize("Where's the  bank?"), "where's the bank");
        assert_eq!(
            normalize("where's the bank"),
            normalize("Where's the Bank!")
        );
    }
}
