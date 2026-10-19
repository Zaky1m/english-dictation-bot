use super::smallcaps::small_caps;
use anyhow::Context;
use std::collections::HashMap;

type Table = HashMap<String, String>;

include!(concat!(env!("OUT_DIR"), "/text_files.rs"));

/// Bot messages per language, one `texts/<code>.toml` each. Keys are the same in
/// every file (checked by a test).
pub struct Texts {
    tables: HashMap<String, Table>,
}

impl Texts {
    pub fn load() -> anyhow::Result<Self> {
        let mut tables = HashMap::new();
        for (code, source) in TEXT_FILES {
            let table = toml::from_str(source).with_context(|| format!("texts/{code}.toml"))?;
            tables.insert(code.to_string(), table);
        }
        Ok(Self { tables })
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
        let lang = if self.tables.contains_key(lang) {
            lang
        } else {
            crate::languages::default_code()
        };
        // Small caps is a style for English only; other Latin scripts stay as written.
        let english = lang == "en";
        let Some(template) = self.tables.get(lang).and_then(|t| t.get(key)) else {
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
    fn keys(&self, lang: &str) -> std::collections::BTreeSet<&String> {
        self.tables[lang].keys().collect()
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
    fn every_language_has_the_same_keys_as_english() {
        let texts = Texts::load().unwrap();
        for (code, _) in TEXT_FILES {
            assert_eq!(texts.keys(code), texts.keys("en"), "texts/{code}.toml");
        }
    }

    #[test]
    fn bot_texts_match_the_language_list() {
        let files: std::collections::BTreeSet<_> = TEXT_FILES.iter().map(|(c, _)| *c).collect();
        let listed: std::collections::BTreeSet<_> = crate::languages::all()
            .iter()
            .map(|l| l.code.as_str())
            .collect();
        assert_eq!(
            files, listed,
            "texts/*.toml and web/i18n/languages.json differ"
        );
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
