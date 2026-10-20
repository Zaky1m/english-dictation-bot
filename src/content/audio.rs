use super::speaker::Speaker;
use anyhow::Context;
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinSet;

/// The voice chosen in the design phase.
pub const VOICE: &str = "Aoede";
const PARALLEL: usize = 4;

/// A pass that records nothing waits this long before the next try, and the
/// task gives up after this many such passes in a row (about a day and a half,
/// longer than a daily free-tier quota takes to come back).
const IDLE_WAIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
const MAX_IDLE_PASSES: u32 = 72;

/// Keeps recording until nothing is missing. Quota errors only pause the task.
pub async fn run_until_done(
    pool: &PgPool,
    speaker: Arc<Speaker>,
    data_dir: &Path,
) -> anyhow::Result<()> {
    let mut idle = 0;
    loop {
        let (done, failed) = pass(pool, speaker.clone(), data_dir).await?;
        if failed == 0 {
            return Ok(());
        }
        idle = if done > 0 { 0 } else { idle + 1 };
        if idle >= MAX_IDLE_PASSES {
            anyhow::bail!("{failed} audio files still fail after {MAX_IDLE_PASSES} tries");
        }
        if done == 0 {
            tracing::info!(
                failed,
                "no audio recorded in this pass, waiting before the next"
            );
            tokio::time::sleep(IDLE_WAIT).await;
        }
    }
}

/// Records every missing sentence and word once. Re-running it only touches
/// what is missing.
pub async fn run(pool: &PgPool, speaker: Arc<Speaker>, data_dir: &Path) -> anyhow::Result<()> {
    let (_, failed) = pass(pool, speaker, data_dir).await?;
    if failed > 0 {
        anyhow::bail!("{failed} audio files failed; run the command again to retry them");
    }
    Ok(())
}

/// Marks sentences and words whose MP3 is already on disk, for example files
/// uploaded by hand or recorded elsewhere, so they are not recorded again.
pub async fn adopt_existing(pool: &PgPool, data_dir: &Path) -> anyhow::Result<u64> {
    let sentences: Vec<i32> = sqlx::query_scalar("SELECT id FROM sentences WHERE NOT audio_ready")
        .fetch_all(pool)
        .await?;
    let words: Vec<String> = sqlx::query_scalar("SELECT word FROM words WHERE NOT audio_ready")
        .fetch_all(pool)
        .await?;
    let mut adopted = 0;
    for id in sentences {
        if non_empty(&data_dir.join("audio/s").join(format!("{id}.mp3"))).await {
            adopted += sqlx::query("UPDATE sentences SET audio_ready = true WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected();
        }
    }
    for word in words {
        if non_empty(&data_dir.join("audio/w").join(format!("{word}.mp3"))).await {
            adopted += sqlx::query("UPDATE words SET audio_ready = true WHERE word = $1")
                .bind(&word)
                .execute(pool)
                .await?
                .rows_affected();
        }
    }
    if adopted > 0 {
        tracing::info!(adopted, "audio files found on disk");
    }
    Ok(adopted)
}

async fn non_empty(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok_and(|m| m.len() > 0)
}

async fn pass(
    pool: &PgPool,
    speaker: Arc<Speaker>,
    data_dir: &Path,
) -> anyhow::Result<(usize, usize)> {
    for sub in ["s", "w"] {
        tokio::fs::create_dir_all(data_dir.join("audio").join(sub)).await?;
    }
    adopt_existing(pool, data_dir).await?;
    let sentences: Vec<(i32, String)> = sqlx::query_as(
        "SELECT id, text FROM sentences WHERE active AND NOT audio_ready ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let words: Vec<(String,)> =
        sqlx::query_as("SELECT word FROM words WHERE NOT audio_ready ORDER BY word")
            .fetch_all(pool)
            .await?;
    tracing::info!(
        sentences = sentences.len(),
        words = words.len(),
        "audio to generate"
    );

    let jobs: Vec<Job> = sentences
        .into_iter()
        .map(|(id, text)| Job {
            speak: text,
            path: data_dir.join("audio/s").join(format!("{id}.mp3")),
            mark: Mark::Sentence(id),
        })
        .chain(words.into_iter().map(|(word,)| Job {
            speak: word.clone(),
            path: data_dir.join("audio/w").join(format!("{word}.mp3")),
            mark: Mark::Word(word),
        }))
        .collect();

    let mut queue = jobs.into_iter();
    let mut tasks = JoinSet::new();
    let (mut done, mut failed) = (0usize, 0usize);
    loop {
        while tasks.len() < PARALLEL {
            let Some(job) = queue.next() else { break };
            let (pool, speaker) = (pool.clone(), speaker.clone());
            tasks.spawn(async move { process(&pool, &speaker, job).await });
        }
        let Some(joined) = tasks.join_next().await else {
            break;
        };
        match joined.context("audio task panicked")? {
            Ok(()) => {
                done += 1;
                if done % 100 == 0 {
                    tracing::info!(done, "audio progress");
                }
            }
            Err(e) => {
                failed += 1;
                tracing::error!(error = %format!("{e:#}"), "audio failed");
            }
        }
    }
    tracing::info!(done, failed, voice = speaker.name(), "audio pass finished");
    Ok((done, failed))
}

struct Job {
    speak: String,
    path: PathBuf,
    mark: Mark,
}

enum Mark {
    Sentence(i32),
    Word(String),
}

async fn process(pool: &PgPool, speaker: &Speaker, job: Job) -> anyhow::Result<()> {
    let mp3 = speaker
        .speak(&job.speak)
        .await
        .with_context(|| format!("speaking {:?}", job.speak))?;
    // Write to a temp name first so a crash never leaves a half file that looks complete.
    let tmp = job.path.with_extension("part");
    tokio::fs::write(&tmp, &mp3).await?;
    tokio::fs::rename(&tmp, &job.path).await?;
    match job.mark {
        Mark::Sentence(id) => {
            sqlx::query("UPDATE sentences SET audio_ready = true WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?
        }
        Mark::Word(word) => {
            sqlx::query("UPDATE words SET audio_ready = true WHERE word = $1")
                .bind(word)
                .execute(pool)
                .await?
        }
    };
    Ok(())
}

/// Removes the start pop and end burst from MP3 files made before the cleaner
/// existed. Originals are copied to `audio-orig/` first; files that need no
/// change are left untouched so repeated runs do not re-encode them.
pub async fn repair(data_dir: &Path) -> anyhow::Result<()> {
    use crate::gemini::{mp3, pcm};
    let (mut changed, mut untouched) = (0usize, 0usize);
    for sub in ["s", "w"] {
        let dir = data_dir.join("audio").join(sub);
        let backup = data_dir.join("audio-orig").join(sub);
        tokio::fs::create_dir_all(&backup).await?;
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "mp3") {
                continue;
            }
            let original = tokio::fs::read(&path).await?;
            let raw = mp3::mp3_to_pcm(&original).await?;
            let cleaned = pcm::clean(&raw);
            if cleaned.len() == raw.len() {
                untouched += 1;
                continue;
            }
            let name = path.file_name().context("file name")?;
            tokio::fs::write(backup.join(name), &original).await?;
            let fixed = mp3::pcm_to_mp3(cleaned).await?;
            let tmp = path.with_extension("part");
            tokio::fs::write(&tmp, &fixed).await?;
            tokio::fs::rename(&tmp, &path).await?;
            changed += 1;
        }
    }
    tracing::info!(changed, untouched, "repair finished");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db;

    #[tokio::test]
    async fn files_placed_by_hand_are_adopted_once() {
        let Some((pool, schema)) = db::test_pool().await else {
            return;
        };
        sqlx::query("INSERT INTO situations (slug, name_ar, name_en, icon, position) VALUES ('t', 'ت', 'T', 'chat', 0)")
            .execute(&pool).await.unwrap();
        let lesson: i32 = sqlx::query_scalar(
            "INSERT INTO lessons (situation_id, number) SELECT id, 1 FROM situations RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let id: i32 = sqlx::query_scalar("INSERT INTO sentences (lesson_id, position, text, normalized, translation, difficulty, tokens) VALUES ($1, 1, 'Hi there', 'hi there', 'مرحبا', 1, '[]') RETURNING id")
            .bind(lesson).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO words (word) VALUES ('hi'), ('there')")
            .execute(&pool)
            .await
            .unwrap();

        let dir = std::env::temp_dir().join(format!("eng-adopt-{schema}"));
        for sub in ["s", "w"] {
            tokio::fs::create_dir_all(dir.join("audio").join(sub))
                .await
                .unwrap();
        }
        tokio::fs::write(dir.join(format!("audio/s/{id}.mp3")), b"mp3")
            .await
            .unwrap();
        tokio::fs::write(dir.join("audio/w/hi.mp3"), b"mp3")
            .await
            .unwrap();
        // An empty file is a failed upload, not audio.
        tokio::fs::write(dir.join("audio/w/there.mp3"), b"")
            .await
            .unwrap();

        assert_eq!(super::adopt_existing(&pool, &dir).await.unwrap(), 2);
        assert_eq!(super::adopt_existing(&pool, &dir).await.unwrap(), 0);
        let missing: i64 = sqlx::query_scalar("SELECT count(*) FROM words WHERE NOT audio_ready")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(missing, 1);

        tokio::fs::remove_dir_all(&dir).await.unwrap();
        db::drop_test_schema(&pool, &schema).await;
    }
}
