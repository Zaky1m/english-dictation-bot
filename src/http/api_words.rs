use super::AppState;
use super::api_exercise::word_audio_url;
use super::error::{ApiError, ApiResult};
use super::user::User;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;

#[derive(Serialize, sqlx::FromRow)]
pub struct SavedWord {
    word: String,
    meaning_ar: Option<String>,
    #[sqlx(skip)]
    audio_url: String,
}

pub async fn save(
    State(state): State<AppState>,
    user: User,
    Path(word): Path<String>,
) -> Result<StatusCode, ApiError> {
    let word = word.to_lowercase();
    let known: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM words WHERE word = $1)")
        .bind(&word)
        .fetch_one(&state.pool)
        .await?;
    if !known {
        return Err(ApiError::NotFound);
    }
    sqlx::query("INSERT INTO saved_words (user_id, word) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user.id)
        .bind(&word)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list(State(state): State<AppState>, user: User) -> ApiResult<Vec<SavedWord>> {
    let mut words: Vec<SavedWord> = sqlx::query_as(
        "SELECT s.word, (SELECT m.meaning_ar FROM sentence_words m WHERE m.word = s.word LIMIT 1) AS meaning_ar
         FROM saved_words s WHERE s.user_id = $1 ORDER BY s.saved_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    for w in &mut words {
        w.audio_url = word_audio_url(&w.word);
    }
    Ok(Json(words))
}
