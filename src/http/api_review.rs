use super::AppState;
use super::api_exercise::{Kind, start_attempt};
use super::error::{ApiError, ApiResult};
use super::user::User;
use crate::cairo;
use axum::Json;
use axum::extract::State;
use chrono::{NaiveDate, Utc};
use serde_json::{Value, json};

pub async fn next_review(State(state): State<AppState>, user: User) -> ApiResult<Value> {
    let today = cairo::date(Utc::now());
    let due: Vec<i32> = sqlx::query_scalar(
        "SELECT rq.sentence_id FROM review_queue rq JOIN sentences s ON s.id = rq.sentence_id
         WHERE rq.user_id = $1 AND rq.due_on <= $2 AND s.active
         ORDER BY rq.due_on, rq.sentence_id",
    )
    .bind(user.id)
    .bind(today)
    .fetch_all(&state.pool)
    .await?;
    let Some(&sentence_id) = due.first() else {
        return Ok(Json(json!({ "none": true })));
    };
    let attempt = start_attempt(&state, &user, sentence_id, Kind::Review).await?;
    let mut value = serde_json::to_value(attempt)?;
    value["remaining"] = json!(due.len() - 1);
    Ok(Json(value))
}

/// Today's challenge sentences, picked on first request if the daily job has not run.
async fn challenge_sentences(state: &AppState, date: NaiveDate) -> Result<Vec<i32>, ApiError> {
    let ids = crate::challenge::ensure(&state.pool, &state.settings, date).await?;
    if ids.is_empty() {
        Err(ApiError::NotFound)
    } else {
        Ok(ids)
    }
}

pub async fn challenge_today(State(state): State<AppState>, user: User) -> ApiResult<Value> {
    let date = cairo::date(Utc::now());
    let ids = challenge_sentences(&state, date).await?;
    let (solved, score): (i32, i32) = sqlx::query_as(
        "SELECT solved, score FROM challenge_scores WHERE cairo_date = $1 AND user_id = $2",
    )
    .bind(date)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or((0, 0));
    Ok(Json(
        json!({ "date": date, "total": ids.len(), "solved": solved, "score": score }),
    ))
}

pub async fn next_challenge(State(state): State<AppState>, user: User) -> ApiResult<Value> {
    let date = cairo::date(Utc::now());
    let ids = challenge_sentences(&state, date).await?;
    let solved: Vec<i32> = sqlx::query_scalar(
        "SELECT sentence_id FROM attempts
         WHERE user_id = $1 AND kind = 'challenge' AND cairo_date = $2 AND completed_at IS NOT NULL",
    )
    .bind(user.id)
    .bind(date)
    .fetch_all(&state.pool)
    .await?;
    let Some(&sentence_id) = ids.iter().find(|id| !solved.contains(id)) else {
        return Ok(Json(json!({ "done": true })));
    };
    let attempt = start_attempt(&state, &user, sentence_id, Kind::Challenge(date)).await?;
    Ok(Json(serde_json::to_value(attempt)?))
}
