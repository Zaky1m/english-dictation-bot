use super::AppState;
use super::ads::{add_credit, consume_credit, providers};
use super::api_profile::ads_on;
use super::error::ApiError;
use super::user::{User, is_pro};
use crate::cairo;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct RewardQuery {
    userid: i64,
}

/// Adsgram calls this when a user finished watching an ad. It adds one credit;
/// the client then claims it. The secret in the path authenticates Adsgram.
pub async fn reward(
    State(state): State<AppState>,
    Path(secret): Path<String>,
    Query(query): Query<RewardQuery>,
) -> Result<StatusCode, ApiError> {
    if !super::webhook::secrets_match(&secret, &state.config.adsgram_reward_secret) {
        return Err(ApiError::NotFound);
    }
    if !ads_on(&state) {
        return Ok(StatusCode::OK);
    }
    let mut conn = state.pool.acquire().await?;
    let ttl = state.settings.int("ad_credit_ttl_minutes");
    if !add_credit(&mut conn, query.userid, ttl).await? {
        return Err(ApiError::TooManyRequests);
    }
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct ClaimBody {
    #[serde(rename = "for")]
    purpose: String,
}

/// Turns an ad credit into extra sentences for today. A word reveal needs no
/// claim: the reveal request itself spends the credit.
pub async fn claim(
    State(state): State<AppState>,
    user: User,
    Json(body): Json<ClaimBody>,
) -> Result<Json<Value>, ApiError> {
    if body.purpose != "extra" {
        return Err(ApiError::BadRequest("bad_purpose"));
    }
    if !ads_on(&state) || is_pro(&state.pool, user.id).await? {
        return Err(ApiError::Forbidden("not_available"));
    }
    let today = cairo::date(Utc::now());
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    let used: i32 = sqlx::query_scalar("SELECT COALESCE((SELECT ad_unlocks FROM daily_usage WHERE user_id = $1 AND cairo_date = $2), 0)")
        .bind(user.id)
        .bind(today)
        .fetch_one(&mut *tx)
        .await?;
    if i64::from(used) >= state.settings.int("ad_unlocks_per_day") {
        return Err(ApiError::Forbidden("daily_ad_limit"));
    }
    if !consume_credit(
        &mut tx,
        user.id,
        state.settings.int("ad_credit_ttl_minutes"),
        "extra",
    )
    .await?
    {
        return Err(ApiError::Forbidden("ad_required"));
    }
    sqlx::query(
        "INSERT INTO daily_usage (user_id, cairo_date, started, ad_unlocks) VALUES ($1, $2, 0, 1)
         ON CONFLICT (user_id, cairo_date) DO UPDATE SET ad_unlocks = daily_usage.ad_unlocks + 1",
    )
    .bind(user.id)
    .bind(today)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({ "extra": state.settings.int("ad_extra_sentences") }),
    ))
}

#[derive(Deserialize)]
pub struct ClientRewardBody {
    provider: String,
}

/// Networks other than Adsgram report a watched ad only to the Mini App, so the
/// app reports it here. That cannot be verified, hence the daily cap on top of
/// the usual limits on open credits.
pub async fn client_reward(
    State(state): State<AppState>,
    user: User,
    Json(body): Json<ClientRewardBody>,
) -> Result<StatusCode, ApiError> {
    let known = providers(&state)
        .iter()
        .any(|p| p["name"] == body.provider.as_str());
    if body.provider == "adsgram" || !known {
        return Err(ApiError::BadRequest("bad_provider"));
    }
    if !ads_on(&state) || is_pro(&state.pool, user.id).await? {
        return Err(ApiError::Forbidden("not_available"));
    }
    let today = cairo::date(Utc::now());
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    let used: i32 = sqlx::query_scalar("SELECT COALESCE((SELECT client_ad_credits FROM daily_usage WHERE user_id = $1 AND cairo_date = $2), 0)")
        .bind(user.id)
        .bind(today)
        .fetch_one(&mut *tx)
        .await?;
    if i64::from(used) >= state.settings.int("client_ad_credits_per_day") {
        return Err(ApiError::Forbidden("daily_ad_limit"));
    }
    if !add_credit(
        &mut tx,
        user.id,
        state.settings.int("ad_credit_ttl_minutes"),
    )
    .await?
    {
        return Err(ApiError::TooManyRequests);
    }
    sqlx::query(
        "INSERT INTO daily_usage (user_id, cairo_date, started, client_ad_credits) VALUES ($1, $2, 0, 1)
         ON CONFLICT (user_id, cairo_date) DO UPDATE SET client_ad_credits = daily_usage.client_ad_credits + 1",
    )
    .bind(user.id)
    .bind(today)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
