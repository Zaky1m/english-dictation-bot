use serde::Deserialize;
use std::sync::OnceLock;

/// One interface language. The list in `web/i18n/languages.json` is shared with
/// the Mini App so both sides always agree on what exists.
#[derive(Debug, Deserialize)]
pub struct Language {
    pub code: String,
    pub name: String,
}

pub fn all() -> &'static [Language] {
    static LIST: OnceLock<Vec<Language>> = OnceLock::new();
    LIST.get_or_init(|| {
        serde_json::from_str(include_str!("../web/i18n/languages.json"))
            .expect("web/i18n/languages.json is valid")
    })
}

/// The first language in the list is the fallback for unknown codes.
pub fn default_code() -> &'static str {
    &all()[0].code
}

pub fn is_supported(code: &str) -> bool {
    all().iter().any(|l| l.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn json_keys(path: &Path) -> BTreeSet<String> {
        let text = std::fs::read_to_string(path).unwrap();
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
        map.keys().cloned().collect()
    }

    #[test]
    fn every_language_has_complete_mini_app_strings() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("web/i18n");
        let reference = json_keys(&dir.join("en.json"));
        for lang in all() {
            let keys = json_keys(&dir.join(format!("{}.json", lang.code)));
            let missing: Vec<_> = reference.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&reference).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "web/i18n/{}.json: missing {missing:?}, extra {extra:?}",
                lang.code
            );
        }
    }

    #[test]
    fn codes_are_unique_and_known() {
        let codes: BTreeSet<_> = all().iter().map(|l| l.code.as_str()).collect();
        assert_eq!(codes.len(), all().len());
        assert!(is_supported("en") && is_supported("ar") && !is_supported("xx"));
        assert_eq!(default_code(), "ar");
    }
}
