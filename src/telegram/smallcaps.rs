const SMALL: [char; 26] = [
    'ᴀ', 'ʙ', 'ᴄ', 'ᴅ', 'ᴇ', 'ꜰ', 'ɢ', 'ʜ', 'ɪ', 'ᴊ', 'ᴋ', 'ʟ', 'ᴍ', 'ɴ', 'ᴏ', 'ᴘ', 'ǫ', 'ʀ', 'ѕ',
    'ᴛ', 'ᴜ', 'ᴠ', 'ᴡ', 'x', 'ʏ', 'ᴢ',
];

fn map(c: char) -> char {
    if c.is_ascii_alphabetic() {
        SMALL[usize::from(c.to_ascii_lowercase() as u8 - b'a')]
    } else {
        c
    }
}

/// Renders English bot text in small caps. Left untouched: `<code>` blocks, HTML
/// tags, `{placeholders}`, `/commands`, `@usernames` and links. Run it on the
/// template, then substitute values, so names inserted later stay as written.
pub fn small_caps(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let at_word_start =
            i == 0 || chars[i - 1].is_whitespace() || matches!(chars[i - 1], '(' | '>');
        let end = if c == '<' {
            if starts_with(&chars, i, "<code>") {
                find(&chars, i, "</code>").map(|p| p + "</code>".len())
            } else {
                find(&chars, i, ">").map(|p| p + 1)
            }
        } else if c == '{' {
            find(&chars, i, "}").map(|p| p + 1)
        } else if at_word_start
            && (c == '/' || c == '@')
            && chars.get(i + 1).is_some_and(|n| n.is_ascii_alphanumeric())
        {
            Some(
                i + 1
                    + chars[i + 1..]
                        .iter()
                        .take_while(|n| n.is_ascii_alphanumeric() || **n == '_')
                        .count(),
            )
        } else if at_word_start
            && (starts_with(&chars, i, "http://") || starts_with(&chars, i, "https://"))
        {
            Some(i + chars[i..].iter().take_while(|n| !n.is_whitespace()).count())
        } else {
            None
        };
        match end {
            Some(end) => {
                out.extend(&chars[i..end]);
                i = end;
            }
            None => {
                out.push(map(c));
                i += 1;
            }
        }
    }
    out
}

fn starts_with(chars: &[char], at: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(k, n)| chars.get(at + k) == Some(&n))
}

fn find(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    (from..chars.len()).find(|&p| starts_with(chars, p, needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_letter_in_both_cases() {
        let all = small_caps("abcdefghijklmnopqrstuvwxyz");
        assert_eq!(all, "ᴀʙᴄᴅᴇꜰɢʜɪᴊᴋʟᴍɴᴏᴘǫʀѕᴛᴜᴠᴡxʏᴢ");
        assert_eq!(small_caps("ABCXYZ"), "ᴀʙᴄxʏᴢ");
    }

    #[test]
    fn leaves_digits_punctuation_and_arabic_alone() {
        assert_eq!(small_caps("Day 7, ok! مرحبا"), "ᴅᴀʏ 7, ᴏᴋ! مرحبا");
    }

    #[test]
    fn protects_code_blocks_and_tags() {
        assert_eq!(
            small_caps("use <code>Abc 1</code> now"),
            "ᴜѕᴇ <code>Abc 1</code> ɴᴏᴡ"
        );
        assert_eq!(small_caps("<b>Hi</b>"), "<b>ʜɪ</b>");
        assert_eq!(
            small_caps("<a href=\"https://x.io\">go</a>"),
            "<a href=\"https://x.io\">ɢᴏ</a>"
        );
    }

    #[test]
    fn protects_placeholders() {
        assert_eq!(
            small_caps("Hello {name}, day {streak}"),
            "ʜᴇʟʟᴏ {name}, ᴅᴀʏ {streak}"
        );
    }

    #[test]
    fn protects_commands_usernames_and_links() {
        assert_eq!(
            small_caps("Send /start to @Eng_Bot"),
            "ѕᴇɴᴅ /start ᴛᴏ @Eng_Bot"
        );
        assert_eq!(
            small_caps("See https://example.com/a?b=C now"),
            "ѕᴇᴇ https://example.com/a?b=C ɴᴏᴡ"
        );
    }

    #[test]
    fn slash_inside_a_word_is_ordinary_text() {
        assert_eq!(small_caps("and/or"), "ᴀɴᴅ/ᴏʀ");
    }

    #[test]
    fn unterminated_markers_do_not_panic() {
        assert_eq!(small_caps("a < b"), "ᴀ < ʙ");
        assert_eq!(small_caps("{open"), "{ᴏᴘᴇɴ");
    }
}
