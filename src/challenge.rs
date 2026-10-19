use crate::settings::Settings;
use chrono::{Duration, NaiveDate};
use sqlx::PgPool;

/// The sentences of the daily challenge for `date`, picked now if the job has
/// not done it yet. Sentences used in the last `challenge_repeat_days` are
/// skipped. Empty when the bank has nothing to pick.
pub async fn ensure(
    pool: &PgPool,
    settings: &Settings,
    date: NaiveDate,
) -> Result<Vec<i32>, sqlx::Error> {
    let existing: Option<Vec<i32>> =
        sqlx::query_scalar("SELECT sentence_ids FROM daily_challenges WHERE cairo_date = $1")
            .bind(date)
            .fetch_optional(pool)
            .await?;
    if let Some(ids) = existing {
        return Ok(ids);
    }
    let since = date - Duration::days(settings.int("challenge_repeat_days"));
    let picked: Vec<i32> = sqlx::query_scalar(
        "SELECT id FROM sentences
         WHERE active AND id <> ALL (SELECT unnest(sentence_ids) FROM daily_challenges WHERE cairo_date > $1)
         ORDER BY random() LIMIT $2",
    )
    .bind(since)
    .bind(settings.int("challenge_size"))
    .fetch_all(pool)
    .await?;
    if picked.is_empty() {
        return Ok(picked);
    }
    // Two first requests can race; the primary key keeps the first one.
    sqlx::query("INSERT INTO daily_challenges (cairo_date, sentence_ids) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(date)
        .bind(&picked)
        .execute(pool)
        .await?;
    sqlx::query_scalar("SELECT sentence_ids FROM daily_challenges WHERE cairo_date = $1")
        .bind(date)
        .fetch_one(pool)
        .await
}
