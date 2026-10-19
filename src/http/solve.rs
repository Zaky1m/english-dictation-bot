use super::AppState;
use crate::cairo;
use crate::exercise::scoring::{Breakdown, Input, next_combo, score};
use crate::review::{self, Action};
use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::{PgConnection, Postgres, Transaction};

pub struct Solved<'a> {
    pub attempt_id: i64,
    pub user_id: i64,
    pub sentence_id: i32,
    pub lesson_id: i32,
    pub kind: &'a str,
    pub cairo_date: Option<NaiveDate>,
    pub letters: usize,
    pub distinct_mistakes: usize,
    pub reveals: usize,
    pub elapsed_ms: i64,
}

#[derive(Serialize)]
pub struct Outcome {
    pub score: Breakdown,
    pub lesson_complete: bool,
    pub meanings: Vec<Meaning>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Meaning {
    pub idx: i16,
    pub word: String,
    pub meaning_ar: String,
}

/// Everything a solved sentence changes, in the caller's transaction so a
/// retry can never score the same attempt twice.
pub async fn finish(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    s: &Solved<'_>,
) -> Result<Outcome, sqlx::Error> {
    let conn: &mut PgConnection = tx;
    let params = state.settings.score_params();

    let previous: i32 = sqlx::query_scalar("SELECT combo FROM users WHERE id = $1 FOR UPDATE")
        .bind(s.user_id)
        .fetch_one(&mut *conn)
        .await?;
    let clean = s.distinct_mistakes == 0 && s.reveals == 0;
    let combo = next_combo(previous.max(0) as u32, clean, params.combo_cap);
    let breakdown = score(
        &Input {
            letters: s.letters,
            distinct_mistakes: s.distinct_mistakes,
            reveals: s.reveals,
            time_ms: s.elapsed_ms,
            combo,
        },
        &params,
    );

    sqlx::query("UPDATE attempts SET completed_at = now(), score = $2, flagged = $3 WHERE id = $1")
        .bind(s.attempt_id)
        .bind(breakdown.score as i32)
        .bind(breakdown.flagged)
        .execute(&mut *conn)
        .await?;
    sqlx::query("UPDATE users SET combo = $2 WHERE id = $1")
        .bind(s.user_id)
        .bind(combo as i32)
        .execute(&mut *conn)
        .await?;

    let mut lesson_complete = false;
    if s.kind == "challenge" {
        if let Some(date) = s.cairo_date {
            sqlx::query(
                "INSERT INTO challenge_scores (cairo_date, user_id, score, solved) VALUES ($1, $2, $3, 1)
                 ON CONFLICT (cairo_date, user_id)
                 DO UPDATE SET score = challenge_scores.score + $3, solved = challenge_scores.solved + 1",
            )
            .bind(date)
            .bind(s.user_id)
            .bind(breakdown.score as i32)
            .execute(&mut *conn)
            .await?;
        }
    } else {
        sqlx::query(
            "INSERT INTO progress (user_id, sentence_id, best_score) VALUES ($1, $2, $3)
             ON CONFLICT (user_id, sentence_id)
             DO UPDATE SET best_score = GREATEST(progress.best_score, $3)",
        )
        .bind(s.user_id)
        .bind(s.sentence_id)
        .bind(breakdown.score as i32)
        .execute(&mut *conn)
        .await?;

        update_review_queue(conn, state, s, clean).await?;

        let done: bool = sqlx::query_scalar(
            "SELECT NOT EXISTS (
                SELECT 1 FROM sentences x
                WHERE x.lesson_id = $2 AND x.active
                  AND NOT EXISTS (SELECT 1 FROM progress p WHERE p.user_id = $1 AND p.sentence_id = x.id))",
        )
        .bind(s.user_id)
        .bind(s.lesson_id)
        .fetch_one(&mut *conn)
        .await?;
        if done {
            lesson_complete = sqlx::query(
                "INSERT INTO lesson_progress (user_id, lesson_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(s.user_id)
            .bind(s.lesson_id)
            .execute(&mut *conn)
            .await?
            .rows_affected()
                > 0;
        }
    }

    let meanings: Vec<Meaning> = sqlx::query_as(
        "SELECT idx, word, meaning_ar FROM sentence_words WHERE sentence_id = $1 ORDER BY idx",
    )
    .bind(s.sentence_id)
    .fetch_all(&mut *conn)
    .await?;

    Ok(Outcome {
        score: breakdown,
        lesson_complete,
        meanings,
    })
}

async fn update_review_queue(
    conn: &mut PgConnection,
    state: &AppState,
    s: &Solved<'_>,
    clean: bool,
) -> Result<(), sqlx::Error> {
    let stage: Option<i16> = sqlx::query_scalar(
        "SELECT stage FROM review_queue WHERE user_id = $1 AND sentence_id = $2 FOR UPDATE",
    )
    .bind(s.user_id)
    .bind(s.sentence_id)
    .fetch_optional(&mut *conn)
    .await?;
    let intervals = state.settings.int_list("review_intervals_days");
    let today = cairo::date(Utc::now());
    match review::after_attempt(stage.map(|v| v as usize), clean, today, &intervals) {
        Some(Action::Graduate) => {
            sqlx::query("DELETE FROM review_queue WHERE user_id = $1 AND sentence_id = $2")
                .bind(s.user_id)
                .bind(s.sentence_id)
                .execute(&mut *conn)
                .await?;
        }
        Some(Action::Schedule { stage, due_on }) => {
            sqlx::query(
                "INSERT INTO review_queue (user_id, sentence_id, stage, due_on) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (user_id, sentence_id) DO UPDATE SET stage = $3, due_on = $4",
            )
            .bind(s.user_id)
            .bind(s.sentence_id)
            .bind(stage as i16)
            .bind(due_on)
            .execute(&mut *conn)
            .await?;
        }
        None => {}
    }
    Ok(())
}
