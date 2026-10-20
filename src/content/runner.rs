//! Long content tasks started from the admin panel or the bot. Progress lives in
//! the `content_jobs` table; only one task runs at a time because they share the
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Write every situation from the built-in list, then make the audio.
    Lessons,
    /// Write the sentences of a situation, then make their audio.
    Generate(i32),
    /// Make audio for everything still marked as missing.
    Audio(Option<i32>),
}

impl Kind {
    fn row(self) -> (&'static str, Option<i32>) {
        match self {
            Self::Lessons => ("lessons", None),
            Self::Generate(id) => ("generate", Some(id)),
            Self::Audio(id) => ("audio", id),
        }
    }

    fn from_row(name: &str, situation: Option<i32>) -> Option<Self> {
        match (name, situation) {
            ("lessons", _) => Some(Self::Lessons),
            ("generate", Some(id)) => Some(Self::Generate(id)),
            ("audio", id) => Some(Self::Audio(id)),
            _ => None,
        }
    }
}

pub async fn start(state: &AppState, kind: Kind) -> Result<i64, StartError> {
    let speaker = super::speaker::Speaker::choose(&state.config, state.gemini.clone())
        .map_err(|e| StartError::Db(format!("{e:#}")))?
        .map(std::sync::Arc::new)
        .ok_or(StartError::NoGemini)?;
    // Writing lessons always needs Gemini; recording audio can use ElevenLabs instead.
    let gemini = match kind {
        Kind::Audio(_) => None,
        _ => Some(state.gemini.clone().ok_or(StartError::NoGemini)?),
    };
    let (name, situation) = kind.row();
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

    let state = state.clone();
    tokio::spawn(async move {
        let (pool, data_dir) = (&state.pool, &state.config.data_dir);
        let outcome = async {
            match (kind, &gemini) {
                (Kind::Lessons, Some(g)) => generate::run(pool, g.clone(), None).await?,
                (Kind::Generate(id), Some(g)) => generate::fill_situation(pool, g, id).await?,
                _ => {}
            }
            audio::run_until_done(pool, speaker, data_dir).await
        }
        .await;
        let (status, detail) = match &outcome {
            Ok(()) => ("done", String::new()),
            Err(e) => ("failed", format!("{e:#}").chars().take(500).collect()),
        };
        if let Err(e) = sqlx::query(
            "UPDATE content_jobs SET status = $2, detail = $3, finished_at = now() WHERE id = $1",
        )
        .bind(job)
        .bind(status)
        .bind(&detail)
        .execute(pool)
        .await
        {
            tracing::error!(job, error = %e, "could not record the job result");
        }
        tell_admins(&state, outcome.is_ok(), &detail).await;
    });
    Ok(job)
}

/// Sends every admin a short note in their language when a task ends.
async fn tell_admins(state: &AppState, ok: bool, detail: &str) {
    for &admin in &state.config.admin_ids {
        let lang: Option<String> = sqlx::query_scalar("SELECT lang FROM users WHERE id = $1")
            .bind(admin)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
        let lang = lang.unwrap_or_else(|| crate::languages::default_code().into());
        let text = if ok {
            state.texts.render(&lang, "job_done", &[])
        } else {
            state
                .texts
                .render(&lang, "job_failed", &[("detail", detail)])
        };
        if let Err(e) = state.bot.send_message(admin, &text, None).await {
            tracing::warn!(admin, error = %e, "could not tell an admin about a finished job");
        }
    }
}

/// A restart kills running tasks without a word. Each one is recorded as
/// interrupted and started again, so a days-long audio run carries on by itself.
pub async fn resume_interrupted(state: &AppState) -> Result<(), sqlx::Error> {
    let rows: Vec<(String, Option<i32>)> = sqlx::query_as(
        "UPDATE content_jobs SET status = 'failed', detail = 'interrupted by a restart', finished_at = now()
         WHERE status = 'running' RETURNING kind, situation_id",
    )
    .fetch_all(&state.pool)
    .await?;
    for (name, situation) in rows {
        let Some(kind) = Kind::from_row(&name, situation) else {
            continue;
        };
        match start(state, kind).await {
            Ok(job) => tracing::info!(job, ?kind, "resumed an interrupted content job"),
            Err(e) => tracing::warn!(?kind, error = ?e, "could not resume a content job"),
        }
    }
    Ok(())
}

/// Counts shown to the owner: how much content exists and how much audio is missing.
pub struct Progress {
    pub situations: i64,
    pub sentences: i64,
    pub sentence_audio_missing: i64,
    pub word_audio_missing: i64,
    pub running: Option<String>,
    pub last_failure: Option<String>,
}

pub async fn progress(pool: &sqlx::PgPool) -> Result<Progress, sqlx::Error> {
    let (situations, sentences, sentence_audio_missing, word_audio_missing): (i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT (SELECT count(*) FROM situations),
                    (SELECT count(*) FROM sentences WHERE active),
                    (SELECT count(*) FROM sentences WHERE active AND NOT audio_ready),
                    (SELECT count(*) FROM words WHERE NOT audio_ready)",
        )
        .fetch_one(pool)
        .await?;
    let running = sqlx::query_scalar(
        "SELECT kind FROM content_jobs WHERE status = 'running' ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    let last_failure = sqlx::query_scalar(
        "SELECT detail FROM content_jobs WHERE status = 'failed' AND finished_at > now() - interval '1 day'
           AND id = (SELECT max(id) FROM content_jobs WHERE status <> 'running')",
    )
    .fetch_optional(pool)
    .await?;
    Ok(Progress {
        situations,
        sentences,
        sentence_audio_missing,
        word_audio_missing,
        running,
        last_failure,
    })
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn kinds_survive_a_round_trip_through_the_table() {
        for kind in [
            Kind::Lessons,
            Kind::Generate(4),
            Kind::Audio(None),
            Kind::Audio(Some(2)),
        ] {
            let (name, situation) = kind.row();
            assert_eq!(Kind::from_row(name, situation), Some(kind));
        }
        assert_eq!(Kind::from_row("generate", None), None);
    }
}
