use crate::gemini::Gemini;
use anyhow::Context;
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinSet;

/// The voice chosen in the design phase.
pub const VOICE: &str = "Aoede";
const PARALLEL: usize = 4;

/// Generates MP3 for every sentence and word that has none yet. Re-running it
/// only touches what is missing.
pub async fn run(pool: &PgPool, gemini: Arc<Gemini>, data_dir: &Path) -> anyhow::Result<()> {
    for sub in ["s", "w"] {
        tokio::fs::create_dir_all(data_dir.join("audio").join(sub)).await?;
    }
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
            let (pool, gemini) = (pool.clone(), gemini.clone());
            tasks.spawn(async move { process(&pool, &gemini, job).await });
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
    tracing::info!(done, failed, "audio finished");
    if failed > 0 {
        anyhow::bail!("{failed} audio files failed; re-run with --missing");
    }
    Ok(())
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

async fn process(pool: &PgPool, gemini: &Gemini, job: Job) -> anyhow::Result<()> {
    let mp3 = gemini
        .speak(&job.speak, VOICE)
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
