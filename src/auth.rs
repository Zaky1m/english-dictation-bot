use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Telegram rejects nothing about age itself; we do.
const MAX_AGE_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("initData has no hash")]
    NoHash,
    #[error("initData signature mismatch")]
    BadSignature,
    #[error("initData is older than 24 hours")]
    Expired,
    #[error("initData has no valid user")]
    NoUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramUser {
    pub id: i64,
    pub name: String,
    pub username: Option<String>,
    pub language_code: Option<String>,
}

#[derive(Deserialize)]
struct RawUser {
    id: i64,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    username: Option<String>,
    language_code: Option<String>,
}

/// Validates the Mini App `initData` string and returns its user.
pub fn validate_init_data(
    init_data: &str,
    bot_token: &str,
    now_unix: i64,
) -> Result<TelegramUser, AuthError> {
    let mut fields: Vec<(String, String)> = form_urlencoded::parse(init_data.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let hash = fields
        .iter()
        .find(|(k, _)| k == "hash")
        .map(|(_, v)| v.clone())
        .ok_or(AuthError::NoHash)?;
    fields.retain(|(k, _)| k != "hash");
    fields.sort();
    let data_check_string = fields
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut key = HmacSha256::new_from_slice(b"WebAppData").map_err(|_| AuthError::BadSignature)?;
    key.update(bot_token.as_bytes());
    let secret = key.finalize().into_bytes();
    let mut mac = HmacSha256::new_from_slice(&secret).map_err(|_| AuthError::BadSignature)?;
    mac.update(data_check_string.as_bytes());
    let expected = hex::decode(&hash).map_err(|_| AuthError::BadSignature)?;
    // verify_slice compares in constant time.
    mac.verify_slice(&expected)
        .map_err(|_| AuthError::BadSignature)?;

    let auth_date: i64 = fields
        .iter()
        .find(|(k, _)| k == "auth_date")
        .and_then(|(_, v)| v.parse().ok())
        .ok_or(AuthError::Expired)?;
    if now_unix - auth_date > MAX_AGE_SECS {
        return Err(AuthError::Expired);
    }

    let raw: RawUser = fields
        .iter()
        .find(|(k, _)| k == "user")
        .and_then(|(_, v)| serde_json::from_str(v).ok())
        .ok_or(AuthError::NoUser)?;
    let name = format!("{} {}", raw.first_name, raw.last_name)
        .trim()
        .to_owned();
    Ok(TelegramUser {
        id: raw.id,
        name,
        username: raw.username,
        language_code: raw.language_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Generated independently with Python's hmac module.
    const TOKEN: &str = "123456:TEST-TOKEN";
    const VECTOR: &str = "auth_date=1700000000&query_id=AAHdF6IQAAAAAN0XohDhrOrc&user=%7B%22id%22%3A4242%2C%22first_name%22%3A%22Test%22%2C%22username%22%3A%22tester%22%2C%22language_code%22%3A%22ar%22%7D&hash=87f011c0e67259521a2f1ac136271e2d9936cd7d044d84db7141e7ceb6025588";
    const NOW: i64 = 1_700_000_100;

    #[test]
    fn accepts_a_valid_signature() {
        let user = validate_init_data(VECTOR, TOKEN, NOW).unwrap();
        assert_eq!(user.id, 4242);
        assert_eq!(user.name, "Test");
        assert_eq!(user.username.as_deref(), Some("tester"));
        assert_eq!(user.language_code.as_deref(), Some("ar"));
    }

    #[test]
    fn rejects_a_wrong_bot_token() {
        assert_eq!(
            validate_init_data(VECTOR, "999:other", NOW),
            Err(AuthError::BadSignature)
        );
    }

    #[test]
    fn rejects_tampered_fields() {
        let tampered = VECTOR.replace("4242", "4243");
        assert_eq!(
            validate_init_data(&tampered, TOKEN, NOW),
            Err(AuthError::BadSignature)
        );
    }

    #[test]
    fn rejects_old_auth_date() {
        assert_eq!(
            validate_init_data(VECTOR, TOKEN, 1_700_000_000 + 24 * 3600 + 1),
            Err(AuthError::Expired)
        );
        assert!(validate_init_data(VECTOR, TOKEN, 1_700_000_000 + 24 * 3600).is_ok());
    }

    #[test]
    fn rejects_missing_hash_and_garbage() {
        let no_hash = VECTOR.split("&hash=").next().unwrap();
        assert_eq!(
            validate_init_data(no_hash, TOKEN, NOW),
            Err(AuthError::NoHash)
        );
        assert_eq!(validate_init_data("", TOKEN, NOW), Err(AuthError::NoHash));
        let bad_hex = VECTOR.replace("87f011", "zzzzzz");
        assert_eq!(
            validate_init_data(&bad_hex, TOKEN, NOW),
            Err(AuthError::BadSignature)
        );
    }
}
