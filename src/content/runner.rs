//! Long content tasks started from the admin panel. Progress lives in the
//! `content_jobs` table; only one task runs at a time because they share the
//! Gemini quota.
use super::{audio, generate};
use crate::http::AppState;

#[derive(Debug, PartialEq, Eq)]
pub enum StartError {
    NoGemini,
    Busy,
    Db(String),
}

impl From<sqlx::Error> for StartError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e.to_string())
    }
}

#[derive(Clone, Copy)]
pub enum Kind {
    /// Write the sentences of a situation, then make their audio.
    Generate(i32),
    /// Make audio for everything still marked as missing.
    Audio(Option<i32>),
}

pub async fn start(state: &AppState, kind: Kind) -> Result<i64, StartError> {
    let gemini = state.gemini.clone().ok_or(StartError::NoGemini)?;
    let (name, situation) = match kind {
        Kind::Generate(id) => ("generate", Some(id)),
        Kind::Audio(id) => ("audio", id),
    };
    let mut tx = state.pool.begin().await?;
    // Serialize starts so two clicks cannot both pass the "nothing running" check.
    sqlx::query("SELECT pg_advisory_xact_lock(7001)")
        .execute(&mut *tx)
        .await?;
    let running: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content_jobs WHERE status = 'running'")
            .fetch_one(&mut *tx)
            .await?;
    if running > 0 {
        return Err(StartError::Busy);
    }
    let job: i64 = sqlx::query_scalar("INSERT INTO content_jobs (kind, situation_id, status) VALUES ($1, $2, 'running') RETURNING id")
        .bind(name)
        .bind(situation)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;

    let (pool, data_dir) = (state.pool.clone(), state.config.data_dir.clone());
    tokio::spawn(async move {
        let outcome = async {
            if let Kind::Generate(id) = kind {
                generate::fill_situation(&pool, &gemini, id).await?;
            }
            audio::run(&pool, gemini.clone(), &data_dir).await
        }
        .await;
        let (status, detail) = match outcome {
            Ok(()) => ("done", String::new()),
            Err(e) => ("failed", format!("{e:#}")),
        };
        if let Err(e) = sqlx::query(
            "UPDATE content_jobs SET status = $2, detail = $3, finished_at = now() WHERE id = $1",
        )
        .bind(job)
        .bind(status)
        .bind(detail.chars().take(500).collect::<String>())
        .execute(&pool)
        .await
        {
            tracing::error!(job, error = %e, "could not record the job result");
        }
    });
    Ok(job)
}

/// A restart kills running tasks without a word; mark them so the panel is honest.
pub async fn mark_interrupted(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE content_jobs SET status = 'failed', detail = 'interrupted by a restart', finished_at = now() WHERE status = 'running'")
        .execute(pool)
        .await?;
    Ok(())
}
