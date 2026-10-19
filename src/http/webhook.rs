use super::AppState;
use super::error::ApiError;
use crate::telegram::{self, types::Update};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;

/// Constant-time comparison so response timing never leaks the secret.
fn secrets_match(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

pub async fn receive(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<StatusCode, ApiError> {
    if !secrets_match(&path, &state.config.webhook_path) {
        return Err(ApiError::NotFound);
    }
    let provided = headers
        .get("x-telegram-bot-api-secret-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !secrets_match(provided, &state.config.webhook_secret) {
        return Err(ApiError::Unauthorized);
    }
    let update: Update = match serde_json::from_value(body) {
        Ok(update) => update,
        Err(e) => {
            // An update type we do not model; retrying will not help.
            tracing::warn!(error = %e, "ignored unparseable update");
            return Ok(StatusCode::OK);
        }
    };

    let fresh =
        sqlx::query("INSERT INTO processed_updates (update_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(update.update_id)
            .execute(&state.pool)
            .await?
            .rows_affected();
    if fresh == 0 {
        return Ok(StatusCode::OK);
    }
    let update_id = update.update_id;
    if let Err(e) = telegram::handle(&state, update).await {
        // Forget the id so Telegram's retry is processed instead of skipped.
        sqlx::query("DELETE FROM processed_updates WHERE update_id = $1")
            .bind(update_id)
            .execute(&state.pool)
            .await?;
        return Err(ApiError::Internal(e));
    }
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::secrets_match;

    #[test]
    fn compares_secrets_exactly() {
        assert!(secrets_match("abc", "abc"));
        assert!(!secrets_match("abc", "abd"));
        assert!(!secrets_match("abc", "abcd"));
        assert!(!secrets_match("", "x"));
    }
}
