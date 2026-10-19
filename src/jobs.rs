//! Scheduled work inside `serve`. A single task wakes every 30 seconds and runs
//! whatever is due. Every job records its progress in the database, so a
//! restart or an overlapping tick never does the same work twice.
use crate::badges;
use crate::cairo;
use crate::challenge;
use crate::http::AppState;
use crate::league;
use crate::telegram::bot::{BotError, callback_button, keyboard, web_app_button};
use crate::telegram::texts::escape_html;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Africa::Cairo;
use std::time::Duration as StdDuration;

const TICK: StdDuration = StdDuration::from_secs(30);
/// Telegram allows about 30 messages a second to different users; stay below it.
const SEND_INTERVAL: StdDuration = StdDuration::from_millis(40);

pub fn spawn(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            run_due(&state, Utc::now()).await;
            tokio::time::sleep(TICK).await;
        }
    })
}

pub async fn run_due(state: &AppState, now: DateTime<Utc>) {
    let today = cairo::date(now);
    if let Err(e) = purge(&state.pool, state.settings.int("ad_credit_ttl_minutes")).await {
        tracing::error!(error = %e, "purge failed");
    }
    if let Err(e) = process_finished_weeks(state, today).await {
        tracing::error!(error = %format!("{e:#}"), "league week processing failed");
    }
    if let Err(e) = challenge::ensure(&state.pool, &state.settings, today).await {
        tracing::error!(error = %e, "daily challenge pick failed");
    }
    if state.settings.bool("challenge_post_enabled")
        && let Err(e) = post_challenge_summary(state, now).await
    {
        tracing::error!(error = %format!("{e:#}"), "challenge summary failed");
    }
    if state.settings.bool("reminders_enabled")
        && let Err(e) = send_reminders(state, now).await
    {
        tracing::error!(error = %format!("{e:#}"), "reminders failed");
    }
}

/// Deletes state that is only meant to live for a day or less.
pub async fn purge(pool: &sqlx::PgPool, ad_credit_ttl_minutes: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM attempts WHERE started_at < now() - interval '24 hours'")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM processed_updates WHERE created_at < now() - interval '24 hours'")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM ad_credits WHERE claimed_at IS NULL AND created_at < now() - make_interval(mins => $1)")
        .bind(ad_credit_ttl_minutes as i32)
        .execute(pool)
        .await?;
    Ok(())
}

async fn process_finished_weeks(state: &AppState, today: NaiveDate) -> anyhow::Result<()> {
    let weeks: Vec<NaiveDate> =
        sqlx::query_scalar("SELECT week_start FROM league_weeks WHERE NOT processed AND week_start + 7 <= $1 ORDER BY week_start")
            .bind(today)
            .fetch_all(&state.pool)
            .await?;
    for week in weeks {
        let moved = process_week(state, week).await?;
        tracing::info!(%week, moved, "league week processed");
    }
    Ok(())
}

/// Promotes and demotes every group of a finished week. The `processed` flag
/// is claimed first inside the transaction, so running it twice moves nobody twice.
pub async fn process_week(state: &AppState, week: NaiveDate) -> Result<usize, sqlx::Error> {
    let mut tx = state.pool.begin().await?;
    let claimed = sqlx::query("UPDATE league_weeks SET processed = true, processed_at = now() WHERE week_start = $1 AND NOT processed")
        .bind(week)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if claimed == 0 {
        return Ok(0);
    }
    let top = state.settings.int("league_promote_top") as usize;
    let bottom = state.settings.int("league_demote_bottom") as usize;
    let groups: Vec<(i64, i16)> =
        sqlx::query_as("SELECT id, tier FROM league_groups WHERE week_start = $1")
            .bind(week)
            .fetch_all(&mut *tx)
            .await?;
    let mut moved = 0;
    for (group, tier) in groups {
        let members: Vec<(i64, i32)> =
            sqlx::query_as("SELECT user_id, points FROM league_members WHERE group_id = $1")
                .bind(group)
                .fetch_all(&mut *tx)
                .await?;
        for (user, new_tier) in league::outcomes(&members, tier, top, bottom) {
            if new_tier == tier {
                continue;
            }
            moved += 1;
            sqlx::query("INSERT INTO user_tiers (user_id, tier) VALUES ($1, $2) ON CONFLICT (user_id) DO UPDATE SET tier = $2")
                .bind(user)
                .bind(new_tier)
                .execute(&mut *tx)
                .await?;
            if new_tier > tier {
                sqlx::query("INSERT INTO user_badges (user_id, badge) VALUES ($1, 'first_promotion') ON CONFLICT DO NOTHING")
                    .bind(user)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    debug_assert!(badges::ALL.contains(&"first_promotion"));
    tx.commit().await?;
    Ok(moved)
}

fn cairo_clock(now: DateTime<Utc>) -> (NaiveDate, NaiveTime) {
    let local = now.with_timezone(&Cairo);
    (local.date_naive(), local.time())
}

async fn post_challenge_summary(state: &AppState, now: DateTime<Utc>) -> anyhow::Result<()> {
    let Some(channel) = state.config.challenge_channel_id.as_deref() else {
        return Ok(());
    };
    let (today, clock) = cairo_clock(now);
    let post_at = state
        .settings
        .text("challenge_post_time")
        .and_then(|t| NaiveTime::parse_from_str(&t, "%H:%M").ok())
        .unwrap_or_else(|| NaiveTime::from_hms_opt(23, 30, 0).unwrap_or_default());
    if clock < post_at {
        return Ok(());
    }
    // Claim first: only one run may post, and a failed send releases the claim.
    let claimed = sqlx::query(
        "UPDATE daily_challenges SET posted_at = now() WHERE cairo_date = $1 AND posted_at IS NULL",
    )
    .bind(today)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if claimed == 0 {
        return Ok(());
    }
    let result = send_summary(state, channel, today).await;
    if result.is_err() {
        sqlx::query("UPDATE daily_challenges SET posted_at = NULL WHERE cairo_date = $1")
            .bind(today)
            .execute(&state.pool)
            .await?;
    }
    result
}

async fn send_summary(state: &AppState, channel: &str, today: NaiveDate) -> anyhow::Result<()> {
    let participants: i64 =
        sqlx::query_scalar("SELECT count(*) FROM challenge_scores WHERE cairo_date = $1")
            .bind(today)
            .fetch_one(&state.pool)
            .await?;
    if participants == 0 {
        return Ok(());
    }
    let top: Vec<(String, i32)> = sqlx::query_as(
        "SELECT left(u.name, 24), c.score FROM challenge_scores c JOIN users u ON u.id = c.user_id
         WHERE c.cairo_date = $1 ORDER BY c.score DESC, c.user_id LIMIT 10",
    )
    .bind(today)
    .fetch_all(&state.pool)
    .await?;
    let lines = top
        .iter()
        .enumerate()
        .map(|(i, (name, score))| format!("• {}. {} ↢ {score}", i + 1, escape_html(name)))
        .collect::<Vec<_>>()
        .join("\n");
    let word = hardest_word(&state.pool, today)
        .await?
        .unwrap_or_else(|| "-".into());
    let text = state.texts.render(
        "ar",
        "challenge_post",
        &[
            ("participants", &participants.to_string()),
            ("word", &word),
            ("top", &lines),
        ],
    );
    // `{top}` already holds escaped HTML-safe text; the template substitution escapes it
    // again, so put the lines back unescaped.
    let text = text.replace(&escape_html(&lines), &lines);
    let kb = keyboard(vec![vec![web_app_button(
        &state.texts.render("ar", "open_app", &[]),
        &state.config.public_url,
        "primary",
    )]]);
    let chat: serde_json::Value = match channel.parse::<i64>() {
        Ok(id) => id.into(),
        Err(_) => channel.into(),
    };
    state
        .bot
        .call(
            "sendMessage",
            &serde_json::json!({ "chat_id": chat, "text": text, "parse_mode": "HTML", "reply_markup": kb }),
        )
        .await?;
    Ok(())
}

/// The word with the most mistakes across today's challenge attempts.
async fn hardest_word(
    pool: &sqlx::PgPool,
    today: NaiveDate,
) -> Result<Option<String>, sqlx::Error> {
    use crate::exercise::tokens::Token;
    use sqlx::types::Json;
    type Row = (Json<Vec<(usize, usize, String)>>, Json<Vec<Token>>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT a.mistakes, s.tokens FROM attempts a JOIN sentences s ON s.id = a.sentence_id
         WHERE a.kind = 'challenge' AND a.cairo_date = $1",
    )
    .bind(today)
    .fetch_all(pool)
    .await?;
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for (mistakes, tokens) in rows {
        let words: std::collections::BTreeSet<&str> = mistakes
            .0
            .iter()
            .filter_map(|(w, _, _)| tokens.0.get(*w).map(|t| t.word.as_str()))
            .collect();
        for word in words {
            *counts.entry(word.to_owned()).or_default() += 1;
        }
    }
    Ok(counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(w, _)| w))
}

async fn send_reminders(state: &AppState, now: DateTime<Utc>) -> anyhow::Result<()> {
    let (today, clock) = cairo_clock(now);
    if i64::from(chrono::Timelike::hour(&clock)) != state.settings.int("reminder_hour") {
        return Ok(());
    }
    let goal = state.settings.int("streak_goal");
    let due: Vec<(i64, String, i32, i64)> = sqlx::query_as(
        "SELECT u.id, u.lang, s.current,
                $2 - COALESCE((SELECT d.sentences FROM daily_stats d WHERE d.user_id = u.id AND d.cairo_date = $1), 0)::bigint
         FROM users u JOIN streaks s ON s.user_id = u.id
         WHERE u.reminders_on AND NOT u.blocked AND NOT u.bot_blocked
           AND s.current > 0 AND s.last_day = $1 - 1
           AND COALESCE((SELECT d.sentences FROM daily_stats d WHERE d.user_id = u.id AND d.cairo_date = $1), 0) < $2
           AND NOT EXISTS (SELECT 1 FROM reminder_log r WHERE r.user_id = u.id AND r.cairo_date = $1)
         ORDER BY u.id",
    )
    .bind(today)
    .bind(goal)
    .fetch_all(&state.pool)
    .await?;
    for (user, lang, streak, left) in due {
        // Record before sending so a crash mid-way never sends twice.
        let fresh = sqlx::query(
            "INSERT INTO reminder_log (user_id, cairo_date) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user)
        .bind(today)
        .execute(&state.pool)
        .await?
        .rows_affected();
        if fresh == 0 {
            continue;
        }
        let text = state.texts.render(
            &lang,
            "reminder",
            &[("streak", &streak.to_string()), ("left", &left.to_string())],
        );
        let kb = keyboard(vec![
            vec![web_app_button(
                &state.texts.render(&lang, "open_app", &[]),
                &state.config.public_url,
                "primary",
            )],
            vec![callback_button(
                &state.texts.render(&lang, "turn_off_reminders", &[]),
                "rem:off",
                "danger",
            )],
        ]);
        match state.bot.send_message(user, &text, Some(kb)).await {
            Ok(()) => {}
            Err(BotError::Blocked) => {
                sqlx::query("UPDATE users SET bot_blocked = true WHERE id = $1")
                    .bind(user)
                    .execute(&state.pool)
                    .await?;
            }
            Err(e) => {
                tracing::warn!(user, error = %e, "reminder not delivered");
                sqlx::query("DELETE FROM reminder_log WHERE user_id = $1 AND cairo_date = $2")
                    .bind(user)
                    .bind(today)
                    .execute(&state.pool)
                    .await?;
            }
        }
        tokio::time::sleep(SEND_INTERVAL).await;
    }
    Ok(())
}
