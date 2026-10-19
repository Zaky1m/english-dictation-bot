use super::smallcaps::small_caps;
use anyhow::Context;
use std::collections::HashMap;

type Table = HashMap<String, String>;

/// Bot messages per language. Keys are the same in every file (checked by a test).
pub struct Texts {
    ar: Table,
    en: Table,
}

impl Texts {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            ar: toml::from_str(include_str!("../../texts/ar.toml")).context("texts/ar.toml")?,
            en: toml::from_str(include_str!("../../texts/en.toml")).context("texts/en.toml")?,
        })
    }

    /// Renders `key` for `lang`, then fills `{placeholders}` with HTML-escaped
    /// values. English goes through small caps before the values go in.
    pub fn render(&self, lang: &str, key: &str, values: &[(&str, &str)]) -> String {
        self.render_with(lang, key, values, &[])
    }

    /// Like `render`, plus `raw` values that are inserted as they are because the
    /// caller already made them safe HTML (for example a list of escaped names).
    pub fn render_with(
        &self,
        lang: &str,
        key: &str,
        values: &[(&str, &str)],
        raw: &[(&str, &str)],
    ) -> String {
        let english = lang == "en";
        let table = if english { &self.en } else { &self.ar };
        let Some(template) = table.get(key) else {
            tracing::error!(key, lang, "missing bot text");
            return String::new();
        };
        let mut text = if english {
            small_caps(template)
        } else {
            template.clone()
        };
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), &escape_html(value));
        }
        for (name, value) in raw {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    #[cfg(test)]
    fn keys(
        &self,
    ) -> (
        std::collections::BTreeSet<&String>,
        std::collections::BTreeSet<&String>,
    ) {
        (self.ar.keys().collect(), self.en.keys().collect())
    }
}

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_have_the_same_keys() {
        let texts = Texts::load().unwrap();
        let (ar, en) = texts.keys();
        assert_eq!(ar, en);
    }

    #[test]
    fn english_is_small_caps_but_names_are_not() {
        let t = Texts::load().unwrap();
        let text = t.render("en", "welcome", &[("name", "Ali <b>")]);
        assert!(text.starts_with("⌯ ᴡᴇʟᴄᴏᴍᴇ Ali &lt;b&gt;"));
        assert!(text.contains("• ʟɪѕᴛᴇɴ"));
    }

    #[test]
    fn arabic_is_left_alone_and_unknown_language_falls_back_to_arabic() {
        let t = Texts::load().unwrap();
        assert!(
            t.render("ar", "welcome", &[("name", "علي")])
                .starts_with("⌯ أهلًا علي")
        );
        assert_eq!(
            t.render("fr", "open_app", &[]),
            t.render("ar", "open_app", &[])
        );
    }

    #[test]
    fn every_message_uses_the_agreed_markers() {
        let t = Texts::load().unwrap();
        for key in [
            "welcome",
            "lang_changed",
            "reminders_off",
            "payment_thanks",
            "unknown",
        ] {
            assert!(t.render("ar", key, &[]).starts_with('⌯'), "{key}");
        }
        assert!(t.render("ar", "lang_changed", &[]).contains("↢"));
        assert!(t.render("en", "lang_changed", &[]).contains("↣"));
    }
}
