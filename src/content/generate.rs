use super::situations::{self, Situation};
use super::validate::{Candidate, validate, word_band};
use crate::exercise::tokens::normalize;
use crate::gemini::Gemini;
use anyhow::Context;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::task::JoinSet;

const LESSONS_PER_SITUATION: i16 = 6;
const SENTENCES_PER_LESSON: i64 = 10;
const MAX_ROUNDS: usize = 6;
const PARALLEL_SITUATIONS: usize = 4;

#[derive(Deserialize)]
struct Batch {
    sentences: Vec<RawSentence>,
}

#[derive(Deserialize)]
struct RawSentence {
    text: String,
    translation: String,
    words: Vec<RawWord>,
}

#[derive(Deserialize)]
struct RawWord {
    word: String,
    meaning_ar: String,
}

fn schema() -> Value {
    json!({
        "type": "OBJECT",
        "properties": { "sentences": { "type": "ARRAY", "items": {
            "type": "OBJECT",
            "properties": {
                "text": { "type": "STRING" },
                "translation": { "type": "STRING" },
                "words": { "type": "ARRAY", "items": {
                    "type": "OBJECT",
                    "properties": {
                        "word": { "type": "STRING" },
                        "meaning_ar": { "type": "STRING" }
                    },
                    "required": ["word", "meaning_ar"]
                } }
            },
            "required": ["text", "translation", "words"]
        } } },
        "required": ["sentences"]
    })
}

fn prompt(situation: &Situation, lesson: i16, count: i64, avoid: &[String]) -> String {
    let (min, max) = word_band(lesson);
    let mut p = format!(
        "Write {count} different natural English sentences for the everyday situation \"{}\" \
         for Arabic-speaking learners of American English (lesson {lesson} of 6, so sentences \
         must be {min} to {max} words long).\n\
         Rules:\n\
         - Only ASCII letters, spaces and these punctuation marks: ' ? . , ! -\n\
         - No digits, no abbreviations with dots, no quotation marks, no emoji.\n\
         - Each sentence is something a real person says in this situation, in standard American English.\n\
         - \"translation\" is a natural Modern Standard Arabic translation.\n\
         - \"words\" lists every whitespace-separated word of the sentence in order, exactly as \
           written (with its punctuation), each with \"meaning_ar\": the Arabic meaning of that single word \
           as used here (one to three Arabic words, no punctuation). Give the word's own meaning even when it is \
           part of a fixed phrase, for example \"good\" is \"جيد\" and \"morning\" is \"صباح\".\n",
        situation.name_en
    );
    if !avoid.is_empty() {
        p.push_str("Do not repeat or closely paraphrase these sentences:\n");
        for s in avoid {
            p.push_str("- ");
            p.push_str(s);
            p.push('\n');
        }
    }
    p
}

pub async fn run(
    pool: &PgPool,
    gemini: Arc<Gemini>,
    slugs: Option<Vec<String>>,
) -> anyhow::Result<()> {
    let all = situations::load()?;
    let ids = upsert_structure(pool, &all).await?;
    let selected: Vec<(Situation, Vec<i32>)> = all
        .into_iter()
        .zip(ids)
        .filter(|(s, _)| slugs.as_ref().is_none_or(|wanted| wanted.contains(&s.slug)))
        .collect();
    if selected.is_empty() {
        anyhow::bail!("no situation matches the given slug");
    }

    let mut tasks = JoinSet::new();
    let permits = Arc::new(tokio::sync::Semaphore::new(PARALLEL_SITUATIONS));
    for (situation, lesson_ids) in selected {
        let (pool, gemini, permits) = (pool.clone(), gemini.clone(), permits.clone());
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await?;
            let slug = situation.slug.clone();
            for (i, lesson_id) in lesson_ids.into_iter().enumerate() {
                let number = i as i16 + 1;
                fill_lesson(&pool, &gemini, &situation, lesson_id, number)
                    .await
                    .with_context(|| format!("{slug} lesson {number}"))?;
            }
            tracing::info!(situation = %slug, "situation complete");
            anyhow::Ok(())
        });
    }
    let mut failed = false;
    while let Some(joined) = tasks.join_next().await {
        if let Err(e) = joined.context("task panicked")? {
            tracing::error!(error = %format!("{e:#}"), "situation failed");
            failed = true;
        }
    }
    if failed {
        anyhow::bail!("some situations failed; re-run to resume");
    }
    Ok(())
}

/// Creates situations and their six lessons if missing; returns lesson ids per situation in order.
async fn upsert_structure(pool: &PgPool, all: &[Situation]) -> anyhow::Result<Vec<Vec<i32>>> {
    let mut result = Vec::new();
    for (position, s) in all.iter().enumerate() {
        let situation_id: i32 = sqlx::query_scalar(
            "INSERT INTO situations (slug, name_ar, name_en, icon, position) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (slug) DO UPDATE SET name_ar = $2, name_en = $3, icon = $4, position = $5
             RETURNING id",
        )
        .bind(&s.slug)
        .bind(&s.name_ar)
        .bind(&s.name_en)
        .bind(&s.icon)
        .bind(position as i16)
        .fetch_one(pool)
        .await?;
        let mut lessons = Vec::new();
        for number in 1..=LESSONS_PER_SITUATION {
            let id: i32 = sqlx::query_scalar(
                "INSERT INTO lessons (situation_id, number) VALUES ($1, $2)
                 ON CONFLICT (situation_id, number) DO UPDATE SET number = $2 RETURNING id",
            )
            .bind(situation_id)
            .bind(number)
            .fetch_one(pool)
            .await?;
            lessons.push(id);
        }
        result.push(lessons);
    }
    Ok(result)
}

async fn fill_lesson(
    pool: &PgPool,
    gemini: &Gemini,
    situation: &Situation,
    lesson_id: i32,
    number: i16,
) -> anyhow::Result<()> {
    for round in 0..MAX_ROUNDS {
        let have: i64 =
            sqlx::query_scalar("SELECT count(*) FROM sentences WHERE lesson_id = $1 AND active")
                .bind(lesson_id)
                .fetch_one(pool)
                .await?;
        if have >= SENTENCES_PER_LESSON {
            return Ok(());
        }
        let avoid: Vec<String> =
            sqlx::query_scalar("SELECT text FROM sentences WHERE lesson_id = $1")
                .bind(lesson_id)
                .fetch_all(pool)
                .await?;
        // Ask for a few extra because some will be rejected.
        let ask = SENTENCES_PER_LESSON - have + 3;
        let value = gemini
            .generate_json(&prompt(situation, number, ask, &avoid), schema())
            .await?;
        let batch: Batch = serde_json::from_value(value).context("unexpected JSON shape")?;
        let accepted = store(pool, lesson_id, number, batch, SENTENCES_PER_LESSON - have).await?;
        tracing::info!(situation = %situation.slug, lesson = number, round, accepted, "round done");
    }
    anyhow::bail!("could not fill the lesson after {MAX_ROUNDS} rounds")
}

/// Drops punctuation the model sometimes copies from the sentence (Arabic and Latin).
fn clean_meaning(meaning: &str) -> &str {
    meaning.trim().trim_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '.' | ',' | '!' | '?' | '\u{060C}' | '\u{061F}' | '\u{061B}' | ':' | ';'
            )
    })
}

async fn store(
    pool: &PgPool,
    lesson_id: i32,
    number: i16,
    batch: Batch,
    room: i64,
) -> anyhow::Result<usize> {
    let mut accepted = 0usize;
    let mut tx = pool.begin().await?;
    let mut position: i16 = sqlx::query_scalar(
        "SELECT COALESCE(max(position), 0)::smallint FROM sentences WHERE lesson_id = $1",
    )
    .bind(lesson_id)
    .fetch_one(&mut *tx)
    .await?;
    for raw in batch.sentences {
        if accepted as i64 >= room {
            break;
        }
        let text = raw.text.trim();
        let meanings: Vec<(String, String)> = raw
            .words
            .into_iter()
            .map(|w| (w.word, w.meaning_ar))
            .collect();
        let candidate = Candidate {
            text,
            translation: &raw.translation,
            meanings: &meanings,
        };
        let tokens = match validate(&candidate, number) {
            Ok(t) => t,
            Err(reason) => {
                tracing::debug!(%reason, text, "rejected");
                continue;
            }
        };
        position += 1;
        let inserted: Option<i32> = sqlx::query_scalar(
            "INSERT INTO sentences (lesson_id, position, text, normalized, translation, difficulty, tokens)
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (normalized) DO NOTHING RETURNING id",
        )
        .bind(lesson_id)
        .bind(position)
        .bind(text)
        .bind(normalize(text))
        .bind(raw.translation.trim())
        .bind(number)
        .bind(serde_json::to_value(&tokens)?)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(sentence_id) = inserted else {
            position -= 1;
            continue;
        };
        for (idx, (token, (_, meaning))) in tokens.iter().zip(&meanings).enumerate() {
            sqlx::query("INSERT INTO words (word) VALUES ($1) ON CONFLICT DO NOTHING")
                .bind(&token.word)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO sentence_words (sentence_id, idx, word, meaning_ar) VALUES ($1, $2, $3, $4)")
                .bind(sentence_id)
                .bind(idx as i16)
                .bind(&token.word)
                .bind(clean_meaning(meaning))
                .execute(&mut *tx)
                .await?;
        }
        accepted += 1;
    }
    tx.commit().await?;
    Ok(accepted)
}

#[cfg(test)]
mod tests {
    use super::clean_meaning;

    #[test]
    fn strips_sentence_punctuation_from_meanings() {
        assert_eq!(clean_meaning(" الخير، "), "الخير");
        assert_eq!(clean_meaning("أراك!"), "أراك");
        assert_eq!(clean_meaning("كيف؟"), "كيف");
        assert_eq!(clean_meaning("جيد"), "جيد");
    }
}
