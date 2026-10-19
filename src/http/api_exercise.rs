use super::AppState;
use super::api_profile::daily_base_limit;
use super::error::{ApiError, ApiResult};
use super::solve::{self, Solved};
use super::user::{User, is_pro};
use crate::cairo;
use crate::exercise::check;
use crate::exercise::tokens::Token;
use axum::Json;
use axum::extract::{Path, State};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::types::Json as SqlJson;
use sqlx::{PgConnection, Postgres, Transaction};
use std::collections::BTreeSet;

#[derive(Serialize)]
pub struct AttemptView {
    attempt_id: i64,
    kind: String,
    sentence_id: i32,
    translation: String,
    /// What the input boxes show: a pattern per word, never the letters.
    tokens: Vec<TokenView>,
    audio_url: String,
    index: Option<i64>,
    total: Option<i64>,
}

#[derive(Serialize)]
struct TokenView {
    pattern: String,
    length: usize,
}

pub fn sentence_audio_url(sentence_id: i32) -> String {
    format!("/audio/s/{sentence_id}.mp3")
}

pub fn word_audio_url(word: &str) -> String {
    format!("/audio/w/{word}.mp3")
}

pub enum Kind {
    Lesson,
    Review,
    Challenge(NaiveDate),
}

impl Kind {
    fn name(&self) -> &'static str {
        match self {
            Self::Lesson => "lesson",
            Self::Review => "review",
            Self::Challenge(_) => "challenge",
        }
    }

    /// The daily challenge does not use up the daily sentence allowance.
    fn counts_toward_limit(&self) -> bool {
        !matches!(self, Self::Challenge(_))
    }
}

/// Opens an attempt for the sentence, or resumes an open one. Starting a new
/// attempt spends one of today's sentences unless the kind is exempt.
pub async fn start_attempt(
    state: &AppState,
    user: &User,
    sentence_id: i32,
    kind: Kind,
) -> Result<AttemptView, ApiError> {
    let pro = is_pro(&state.pool, user.id).await?;
    let mut tx = state.pool.begin().await?;
    // Serialize per user so two taps cannot both pass the limit check.
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    let challenge_date = match kind {
        Kind::Challenge(date) => Some(date),
        _ => None,
    };
    let open: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM attempts
         WHERE user_id = $1 AND sentence_id = $2 AND kind = $3 AND completed_at IS NULL
           AND started_at > now() - interval '24 hours'
           AND ($4::date IS NULL OR cairo_date = $4)
         ORDER BY id DESC LIMIT 1",
    )
    .bind(user.id)
    .bind(sentence_id)
    .bind(kind.name())
    .bind(challenge_date)
    .fetch_optional(&mut *tx)
    .await?;

    let attempt_id = match open {
        Some(id) => id,
        None => {
            let today = cairo::date(Utc::now());
            if kind.counts_toward_limit() {
                spend_sentence(&mut tx, state, user.id, today, pro).await?;
            }
            sqlx::query_scalar(
                "INSERT INTO attempts (user_id, sentence_id, kind, cairo_date) VALUES ($1, $2, $3, $4) RETURNING id",
            )
            .bind(user.id)
            .bind(sentence_id)
            .bind(kind.name())
            .bind(challenge_date.unwrap_or(today))
            .fetch_one(&mut *tx)
            .await?
        }
    };
    tx.commit().await?;
    view(&state.pool, attempt_id, sentence_id, kind.name()).await
}

async fn spend_sentence(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    user_id: i64,
    today: NaiveDate,
    pro: bool,
) -> Result<(), ApiError> {
    let base = daily_base_limit(state, pro);
    let ad_extra = if pro {
        0
    } else {
        state.settings.int("ad_extra_sentences")
    };
    let spent: Option<i32> = sqlx::query_scalar(
        "INSERT INTO daily_usage (user_id, cairo_date, started) VALUES ($1, $2, 1)
         ON CONFLICT (user_id, cairo_date) DO UPDATE SET started = daily_usage.started + 1
           WHERE daily_usage.started < $3 + daily_usage.ad_unlocks * $4
         RETURNING started",
    )
    .bind(user_id)
    .bind(today)
    .bind(base as i32)
    .bind(ad_extra as i32)
    .fetch_optional(&mut **tx)
    .await?;
    spent
        .map(|_| ())
        .ok_or(ApiError::Forbidden("limit_reached"))
}

async fn view(
    pool: &sqlx::PgPool,
    attempt_id: i64,
    sentence_id: i32,
    kind: &str,
) -> Result<AttemptView, ApiError> {
    let (translation, tokens, lesson_id, position): (String, SqlJson<Vec<Token>>, i32, i16) =
        sqlx::query_as(
            "SELECT translation, tokens, lesson_id, position FROM sentences WHERE id = $1",
        )
        .bind(sentence_id)
        .fetch_one(pool)
        .await?;
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sentences WHERE lesson_id = $1 AND active")
            .bind(lesson_id)
            .fetch_one(pool)
            .await?;
    let in_lesson = kind == "lesson";
    Ok(AttemptView {
        attempt_id,
        kind: kind.to_owned(),
        sentence_id,
        translation,
        tokens: tokens
            .0
            .iter()
            .map(|t| TokenView {
                pattern: t.pattern.clone(),
                length: t.letters.len(),
            })
            .collect(),
        audio_url: sentence_audio_url(sentence_id),
        index: in_lesson.then_some(i64::from(position)),
        total: in_lesson.then_some(total),
    })
}

pub async fn next_in_lesson(
    State(state): State<AppState>,
    user: User,
    Path(lesson_id): Path<i32>,
) -> ApiResult<Value> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM lessons WHERE id = $1)")
        .bind(lesson_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    if !is_pro(&state.pool, user.id).await?
        && !lesson_unlocked(&state.pool, user.id, lesson_id).await?
    {
        return Err(ApiError::Forbidden("locked"));
    }
    let next: Option<i32> = sqlx::query_scalar(
        "SELECT x.id FROM sentences x
         WHERE x.lesson_id = $2 AND x.active
           AND NOT EXISTS (SELECT 1 FROM progress p WHERE p.user_id = $1 AND p.sentence_id = x.id)
         ORDER BY x.position LIMIT 1",
    )
    .bind(user.id)
    .bind(lesson_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(sentence_id) = next else {
        return Ok(Json(json!({ "done": true })));
    };
    let attempt = start_attempt(&state, &user, sentence_id, Kind::Lesson).await?;
    Ok(Json(serde_json::to_value(attempt)?))
}

async fn lesson_unlocked(
    pool: &sqlx::PgPool,
    user_id: i64,
    lesson_id: i32,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "WITH ordered AS (
            SELECT l.id, row_number() OVER (ORDER BY s.position, l.number) AS rn
            FROM lessons l JOIN situations s ON s.id = l.situation_id WHERE s.active),
         previous AS (
            SELECT p.id FROM ordered p
            WHERE p.rn = (SELECT rn FROM ordered WHERE id = $2) - 1)
         SELECT NOT EXISTS (SELECT 1 FROM previous)
             OR EXISTS (SELECT 1 FROM lesson_progress lp, previous WHERE lp.user_id = $1 AND lp.lesson_id = previous.id)",
    )
    .bind(user_id)
    .bind(lesson_id)
    .fetch_one(pool)
    .await
}

pub async fn play(
    State(state): State<AppState>,
    user: User,
    Path(attempt_id): Path<i64>,
) -> Result<axum::http::StatusCode, ApiError> {
    let touched = sqlx::query(
        "UPDATE attempts SET audio_started_at = COALESCE(audio_started_at, now())
         WHERE id = $1 AND user_id = $2 AND completed_at IS NULL",
    )
    .bind(attempt_id)
    .bind(user.id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if touched == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct OpenAttempt {
    sentence_id: i32,
    kind: String,
    cairo_date: Option<NaiveDate>,
    mistakes: SqlJson<Vec<(usize, usize, String)>>,
    reveals: Vec<i32>,
    since_check_ms: Option<i64>,
    elapsed_ms: i64,
}

/// Locks the caller's open attempt for the rest of the transaction.
async fn lock_attempt(
    conn: &mut PgConnection,
    attempt_id: i64,
    user_id: i64,
) -> Result<OpenAttempt, ApiError> {
    sqlx::query_as(
        "SELECT sentence_id, kind, cairo_date, mistakes, reveals,
                (extract(epoch FROM now() - last_check_at) * 1000)::bigint AS since_check_ms,
                (extract(epoch FROM now() - COALESCE(audio_started_at, started_at)) * 1000)::bigint AS elapsed_ms
         FROM attempts
         WHERE id = $1 AND user_id = $2 AND completed_at IS NULL AND started_at > now() - interval '24 hours'
         FOR UPDATE",
    )
    .bind(attempt_id)
    .bind(user_id)
    .fetch_optional(conn)
    .await?
    .ok_or(ApiError::NotFound)
}

#[derive(Deserialize)]
pub struct CheckBody {
    words: Vec<String>,
}

pub async fn check(
    State(state): State<AppState>,
    user: User,
    Path(attempt_id): Path<i64>,
    Json(body): Json<CheckBody>,
) -> ApiResult<Value> {
    if body.words.len() > 40 || body.words.iter().any(|w| w.len() > 64) {
        return Err(ApiError::BadRequest("too_long"));
    }
    let mut tx = state.pool.begin().await?;
    let attempt = lock_attempt(&mut tx, attempt_id, user.id).await?;
    let min_interval = state.settings.int("check_min_interval_ms");
    if attempt.since_check_ms.is_some_and(|ms| ms < min_interval) {
        return Err(ApiError::TooManyRequests);
    }

    let (text, translation, tokens, lesson_id): (String, String, SqlJson<Vec<Token>>, i32) =
        sqlx::query_as("SELECT text, translation, tokens, lesson_id FROM sentences WHERE id = $1")
            .bind(attempt.sentence_id)
            .fetch_one(&mut *tx)
            .await?;
    let (result, new_mistakes) = check::check(&tokens.0, &body.words);

    // A mistake counts once per (word, position, letter) for the whole attempt.
    let mut seen: BTreeSet<(usize, usize, String)> = attempt.mistakes.0.into_iter().collect();
    for (w, p, c) in new_mistakes {
        seen.insert((w, p, c.to_string()));
    }
    let seen: Vec<_> = seen.into_iter().collect();
    sqlx::query("UPDATE attempts SET mistakes = $2, last_check_at = now() WHERE id = $1")
        .bind(attempt_id)
        .bind(SqlJson(&seen))
        .execute(&mut *tx)
        .await?;

    if !result.solved {
        tx.commit().await?;
        return Ok(Json(json!({ "solved": false, "words": result.words })));
    }

    let letters: usize = tokens.0.iter().map(|t| t.letters.len()).sum();
    let outcome = solve::finish(
        &mut tx,
        &state,
        &Solved {
            attempt_id,
            user_id: user.id,
            sentence_id: attempt.sentence_id,
            lesson_id,
            kind: &attempt.kind,
            cairo_date: attempt.cairo_date,
            letters,
            distinct_mistakes: seen.len(),
            reveals: attempt.reveals.len(),
            elapsed_ms: attempt.elapsed_ms,
        },
    )
    .await?;
    tx.commit().await?;

    let meanings: Vec<Value> = outcome
        .meanings
        .iter()
        .map(|m| json!({ "idx": m.idx, "word": m.word, "meaning_ar": m.meaning_ar, "audio_url": word_audio_url(&m.word) }))
        .collect();
    Ok(Json(json!({
        "solved": true,
        "words": result.words,
        "sentence": { "text": text, "translation": translation },
        "score": outcome.score,
        "lesson_complete": outcome.lesson_complete,
        "meanings": meanings,
    })))
}

#[derive(Deserialize)]
pub struct RevealBody {
    word_index: usize,
}

pub async fn reveal(
    State(state): State<AppState>,
    user: User,
    Path(attempt_id): Path<i64>,
    Json(body): Json<RevealBody>,
) -> ApiResult<Value> {
    let pro = is_pro(&state.pool, user.id).await?;
    let mut tx = state.pool.begin().await?;
    let attempt = lock_attempt(&mut tx, attempt_id, user.id).await?;
    let tokens: SqlJson<Vec<Token>> =
        sqlx::query_scalar("SELECT tokens FROM sentences WHERE id = $1")
            .bind(attempt.sentence_id)
            .fetch_one(&mut *tx)
            .await?;
    let token = tokens
        .0
        .get(body.word_index)
        .ok_or(ApiError::BadRequest("bad_word_index"))?;

    let index = body.word_index as i32;
    if !attempt.reveals.contains(&index) {
        // Revealing is free for Pro, and for everyone while ads are off.
        if !pro && state.settings.bool("ads_enabled") {
            let credit: Option<i64> = sqlx::query_scalar(
                "UPDATE ad_credits SET claimed_at = now(), claimed_for = 'reveal'
                 WHERE id = (SELECT id FROM ad_credits
                             WHERE user_id = $1 AND claimed_at IS NULL
                               AND created_at > now() - make_interval(mins => $2)
                             ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED)
                 RETURNING id",
            )
            .bind(user.id)
            .bind(state.settings.int("ad_credit_ttl_minutes") as i32)
            .fetch_optional(&mut *tx)
            .await?;
            if credit.is_none() {
                return Err(ApiError::Forbidden("ad_required"));
            }
        }
        sqlx::query("UPDATE attempts SET reveals = array_append(reveals, $2) WHERE id = $1")
            .bind(attempt_id)
            .bind(index)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({
        "word_index": body.word_index,
        "letters": token.letters,
        "audio_url": word_audio_url(&token.word),
    })))
}
