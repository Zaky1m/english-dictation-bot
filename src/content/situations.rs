use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Situation {
    pub slug: String,
    pub name_ar: String,
    pub name_en: String,
    pub icon: String,
}

#[derive(Deserialize)]
struct File {
    situation: Vec<Situation>,
}

/// Order in the file is the order on the map.
pub fn load() -> anyhow::Result<Vec<Situation>> {
    let file: File = toml::from_str(include_str!("situations.toml")).context("situations.toml")?;
    Ok(file.situation)
}

#[cfg(test)]
mod tests {
    #[test]
    fn list_has_twenty_unique_slugs() {
        let all = super::load().unwrap();
        assert_eq!(all.len(), 20);
        let mut slugs: Vec<_> = all.iter().map(|s| s.slug.as_str()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), 20);
    }
}
