use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required variable {0}")]
    Missing(&'static str),
    #[error("invalid value for {name}: {reason}")]
    Invalid { name: &'static str, reason: String },
}

// No Debug derive: the struct holds secrets and must never reach a log line.
#[allow(dead_code, reason = "fields are consumed by later phases")]
#[derive(Clone)]
pub struct Config {
    pub bot_token: String,
    pub public_url: String,
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub webhook_path: String,
    pub webhook_secret: String,
    pub adsgram_reward_secret: String,
    pub adsgram_block_id: Option<String>,
    pub admin_ids: Vec<i64>,
    pub admin_username: Option<String>,
    pub challenge_channel_id: Option<String>,
    pub data_dir: PathBuf,
    pub web_dir: PathBuf,
    pub gemini_keys: Vec<String>,
}

impl Config {
    pub fn gemini_keys_from(vars: impl IntoIterator<Item = (String, String)>) -> Vec<String> {
        gemini_keys(&vars.into_iter().collect())
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_vars(std::env::vars())
    }

    pub fn from_vars(
        vars: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ConfigError> {
        let vars: HashMap<String, String> = vars.into_iter().collect();
        let get = |name: &str| {
            vars.get(name)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let required = |name: &'static str| get(name).ok_or(ConfigError::Missing(name));

        let bind_addr = required("BIND_ADDR")?
            .parse()
            .map_err(|e: std::net::AddrParseError| ConfigError::Invalid {
                name: "BIND_ADDR",
                reason: e.to_string(),
            })?;

        let admin_ids = match get("ADMIN_IDS") {
            Some(raw) => raw
                .split(',')
                .map(|id| {
                    id.trim().parse::<i64>().map_err(|e| ConfigError::Invalid {
                        name: "ADMIN_IDS",
                        reason: e.to_string(),
                    })
                })
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };

        Ok(Self {
            bot_token: required("BOT_TOKEN")?,
            public_url: required("PUBLIC_URL")?.trim_end_matches('/').to_owned(),
            bind_addr,
            database_url: required("DATABASE_URL")?,
            webhook_path: required("WEBHOOK_PATH")?,
            webhook_secret: required("WEBHOOK_SECRET")?,
            adsgram_reward_secret: required("ADSGRAM_REWARD_SECRET")?,
            adsgram_block_id: get("ADSGRAM_BLOCK_ID"),
            admin_ids,
            admin_username: get("ADMIN_USERNAME").map(|u| u.trim_start_matches('@').to_owned()),
            challenge_channel_id: get("CHALLENGE_CHANNEL_ID"),
            data_dir: get("DATA_DIR").unwrap_or_else(|| "data".into()).into(),
            web_dir: get("WEB_DIR").unwrap_or_else(|| "web".into()).into(),
            gemini_keys: gemini_keys(&vars),
        })
    }
}

/// Collects `GEMINI_API_KEY_<n>` values ordered by `n`, so the rotation order
/// is stable no matter how the environment is enumerated.
fn gemini_keys(vars: &HashMap<String, String>) -> Vec<String> {
    let mut keys: Vec<(u32, String)> = vars
        .iter()
        .filter_map(|(name, value)| {
            let n = name.strip_prefix("GEMINI_API_KEY_")?.parse().ok()?;
            let value = value.trim();
            (!value.is_empty()).then(|| (n, value.to_owned()))
        })
        .collect();
    keys.sort_by_key(|(n, _)| *n);
    keys.into_iter().map(|(_, key)| key).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Vec<(String, String)> {
        [
            ("BOT_TOKEN", "1:abc"),
            ("PUBLIC_URL", "https://example.com/"),
            ("BIND_ADDR", "127.0.0.1:4510"),
            ("DATABASE_URL", "postgresql://u:p@127.0.0.1/db"),
            ("WEBHOOK_PATH", "path"),
            ("WEBHOOK_SECRET", "secret"),
            ("ADSGRAM_REWARD_SECRET", "reward"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    fn with(extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut vars = base();
        vars.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        vars
    }

    #[test]
    fn parses_minimal_config_with_defaults() {
        let cfg = Config::from_vars(base()).unwrap();
        assert_eq!(cfg.public_url, "https://example.com");
        assert!(cfg.admin_ids.is_empty());
        assert!(cfg.adsgram_block_id.is_none());
        assert_eq!(cfg.data_dir, PathBuf::from("data"));
        assert!(cfg.gemini_keys.is_empty());
    }

    #[test]
    fn missing_required_variable_is_reported() {
        let vars: Vec<_> = base()
            .into_iter()
            .filter(|(k, _)| k != "BOT_TOKEN")
            .collect();
        let err = Config::from_vars(vars)
            .err()
            .expect("config should be rejected");
        assert!(matches!(err, ConfigError::Missing("BOT_TOKEN")));
    }

    #[test]
    fn empty_optional_values_count_as_unset() {
        let cfg =
            Config::from_vars(with(&[("ADMIN_IDS", " "), ("CHALLENGE_CHANNEL_ID", "")])).unwrap();
        assert!(cfg.admin_ids.is_empty());
        assert!(cfg.challenge_channel_id.is_none());
    }

    #[test]
    fn gemini_keys_are_sorted_numerically() {
        let cfg = Config::from_vars(with(&[
            ("GEMINI_API_KEY_10", "k10"),
            ("GEMINI_API_KEY_2", "k2"),
            ("GEMINI_API_KEY_1", "k1"),
            ("GEMINI_API_KEY_X", "ignored"),
            ("GEMINI_API_KEY_3", ""),
        ]))
        .unwrap();
        assert_eq!(cfg.gemini_keys, ["k1", "k2", "k10"]);
    }

    #[test]
    fn admin_ids_and_username_are_normalised() {
        let cfg =
            Config::from_vars(with(&[("ADMIN_IDS", "5, 7"), ("ADMIN_USERNAME", "@boss")])).unwrap();
        assert_eq!(cfg.admin_ids, [5, 7]);
        assert_eq!(cfg.admin_username.as_deref(), Some("boss"));
    }

    #[test]
    fn invalid_admin_id_is_rejected() {
        let err = Config::from_vars(with(&[("ADMIN_IDS", "5,x")]))
            .err()
            .expect("config should be rejected");
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "ADMIN_IDS",
                ..
            }
        ));
    }

    #[test]
    fn invalid_bind_addr_is_rejected() {
        let mut vars: Vec<_> = base()
            .into_iter()
            .filter(|(k, _)| k != "BIND_ADDR")
            .collect();
        vars.push(("BIND_ADDR".into(), "nope".into()));
        let err = Config::from_vars(vars)
            .err()
            .expect("config should be rejected");
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "BIND_ADDR",
                ..
            }
        ));
    }
}
