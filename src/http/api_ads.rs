use super::AppState;
use super::ads::consume_credit;
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

/// Unclaimed credits a user may hold at once, and the minimum gap between two.
const MAX_OPEN_CREDITS: i64 = 3;
const MIN_GAP_SECS: i32 = 20;

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
    let added = sqlx::query(
        "INSERT INTO ad_credits (user_id)
         SELECT $1 WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)
           AND (SELECT count(*) FROM ad_credits WHERE user_id = $1 AND claimed_at IS NULL
                  AND created_at > now() - make_interval(mins => $2)) < $3
           AND NOT EXISTS (SELECT 1 FROM ad_credits WHERE user_id = $1
                  AND created_at > now() - make_interval(secs => $4))",
    )
    .bind(query.userid)
    .bind(state.settings.int("ad_credit_ttl_minutes") as i32)
    .bind(MAX_OPEN_CREDITS)
    .bind(MIN_GAP_SECS)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if added == 0 {
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
