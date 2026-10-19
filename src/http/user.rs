use super::AppState;
use super::error::ApiError;
use crate::auth::validate_init_data;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::PgPool;

/// The authenticated Mini App user. Reads `Authorization: tma <initData>`.
pub struct User {
    pub id: i64,
    pub lang: String,
}

impl FromRequestParts<AppState> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let init_data = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("tma "))
            .ok_or(ApiError::Unauthorized)?;
        let now = chrono::Utc::now().timestamp();
        let tg = validate_init_data(init_data, &state.config.bot_token, now).map_err(|e| {
            tracing::warn!(reason = %e, "rejected initData");
            ApiError::Unauthorized
        })?;
        let (lang, blocked): (String, bool) = sqlx::query_as(
            "INSERT INTO users (id, name, username) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET name = $2, username = $3
             RETURNING lang, blocked",
        )
        .bind(tg.id)
        .bind(&tg.name)
        .bind(&tg.username)
        .fetch_one(&state.pool)
        .await?;
        if blocked {
            return Err(ApiError::Forbidden("blocked"));
        }
        Ok(Self { id: tg.id, lang })
    }
}

pub async fn is_pro(pool: &PgPool, user_id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM subscriptions WHERE user_id = $1 AND ends_at > now())",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}
