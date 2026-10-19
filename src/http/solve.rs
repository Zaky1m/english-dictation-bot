use super::AppState;
use crate::badges::{self, Snapshot};
use crate::cairo;
use crate::exercise::scoring::{Breakdown, Input, next_combo, score};
use crate::exercise::tokens::Token;
use crate::league;
use crate::review::{self, Action};
use crate::streak::{self, StreakState};
use chrono::Datelike;
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
    pub tokens: &'a [Token],
    /// Distinct wrong letters: (word index, position, typed letter).
    pub mistakes: &'a [(usize, usize, String)],
}

#[derive(Serialize)]
pub struct Outcome {
    pub score: Breakdown,
    pub lesson_complete: bool,
    pub meanings: Vec<Meaning>,
    pub streak: i32,
    pub new_badges: Vec<&'static str>,
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
    sqlx::query(
        "UPDATE users SET combo = $2, stat_solved = stat_solved + 1,
                stat_clean = stat_clean + $3 WHERE id = $1",
    )
    .bind(s.user_id)
    .bind(combo as i32)
    .bind(i32::from(clean))
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
            "INSERT INTO progress (user_id, sentence_id, best_score, clean) VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id, sentence_id)
             DO UPDATE SET best_score = GREATEST(progress.best_score, $3)",
        )
        .bind(s.user_id)
        .bind(s.sentence_id)
        .bind(breakdown.score as i32)
        .bind(clean)
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

    let today = cairo::date(Utc::now());
    let counts_for_goal = s.kind != "challenge";
    let sentences_today =
        record_daily_stats(conn, s, today, breakdown.score, counts_for_goal).await?;
    record_misses(conn, s).await?;

    let mut streak_now = 0;
    if counts_for_goal {
        league_points(conn, state, s.user_id, today, breakdown.score).await?;
        let goal = state.settings.int("streak_goal");
        streak_now = update_streak(conn, state, s.user_id, today, sentences_today >= goal).await?;
    }
    let new_badges = award_badges(conn, s, &breakdown, lesson_complete, streak_now).await?;

    Ok(Outcome {
        score: breakdown,
        lesson_complete,
        meanings,
        streak: streak_now,
        new_badges,
    })
}

/// Counts the day's sentences and points; returns sentences done today.
async fn record_daily_stats(
    conn: &mut PgConnection,
    s: &Solved<'_>,
    today: NaiveDate,
    points: i64,
    counts: bool,
) -> Result<i64, sqlx::Error> {
    let sentences: i32 = sqlx::query_scalar(
        "INSERT INTO daily_stats (user_id, cairo_date, sentences, points) VALUES ($1, $2, $3, $4)
         ON CONFLICT (user_id, cairo_date)
         DO UPDATE SET sentences = daily_stats.sentences + $3, points = daily_stats.points + $4
         RETURNING sentences",
    )
    .bind(s.user_id)
    .bind(today)
    .bind(i32::from(counts))
    .bind(points as i32)
    .fetch_one(&mut *conn)
    .await?;
    Ok(i64::from(sentences))
}

/// Feeds the Pro "most missed" lists: the letter that was expected, and the word.
async fn record_misses(conn: &mut PgConnection, s: &Solved<'_>) -> Result<(), sqlx::Error> {
    let mut words = std::collections::BTreeSet::new();
    for (word_idx, position, _) in s.mistakes {
        let Some(token) = s.tokens.get(*word_idx) else {
            continue;
        };
        // A letter typed past the end of the word has nothing it should have been.
        let Some(expected) = token.letters.chars().nth(*position) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO missed_letters (user_id, letter, count) VALUES ($1, $2, 1)
             ON CONFLICT (user_id, letter) DO UPDATE SET count = missed_letters.count + 1",
        )
        .bind(s.user_id)
        .bind(expected.to_string())
        .execute(&mut *conn)
        .await?;
        words.insert(token.word.clone());
    }
    for word in words {
        sqlx::query(
            "INSERT INTO missed_words (user_id, word, count) VALUES ($1, $2, 1)
             ON CONFLICT (user_id, word) DO UPDATE SET count = missed_words.count + 1",
        )
        .bind(s.user_id)
        .bind(word)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Adds league points, placing the user in a group on their first points of the week.
async fn league_points(
    conn: &mut PgConnection,
    state: &AppState,
    user_id: i64,
    today: NaiveDate,
    points: i64,
) -> Result<(), sqlx::Error> {
    let week = league::week_start(today);
    let updated = sqlx::query(
        "UPDATE league_members SET points = points + $3 WHERE week_start = $1 AND user_id = $2",
    )
    .bind(week)
    .bind(user_id)
    .bind(points as i32)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if updated > 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO league_weeks (week_start) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(week)
        .execute(&mut *conn)
        .await?;
    sqlx::query("INSERT INTO user_tiers (user_id, tier) VALUES ($1, 0) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    let tier: i16 = sqlx::query_scalar("SELECT tier FROM user_tiers WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await?;
    // One writer per (week, tier) so a group never overfills.
    let lock_key = week.num_days_from_ce() as i64 * 10 + i64::from(tier);
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *conn)
        .await?;
    let size = state.settings.int("league_group_size");
    let open: Option<i64> = sqlx::query_scalar(
        "SELECT g.id FROM league_groups g
         WHERE g.week_start = $1 AND g.tier = $2
           AND (SELECT count(*) FROM league_members m WHERE m.group_id = g.id) < $3
         ORDER BY g.id LIMIT 1",
    )
    .bind(week)
    .bind(tier)
    .bind(size)
    .fetch_optional(&mut *conn)
    .await?;
    let group = match open {
        Some(id) => id,
        None => {
            sqlx::query_scalar(
                "INSERT INTO league_groups (week_start, tier) VALUES ($1, $2) RETURNING id",
            )
            .bind(week)
            .bind(tier)
            .fetch_one(&mut *conn)
            .await?
        }
    };
    sqlx::query("INSERT INTO league_members (group_id, week_start, user_id, points) VALUES ($1, $2, $3, $4)")
        .bind(group)
        .bind(week)
        .bind(user_id)
        .bind(points as i32)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Applies the streak rules when the daily goal is met; returns the streak to show.
async fn update_streak(
    conn: &mut PgConnection,
    state: &AppState,
    user_id: i64,
    today: NaiveDate,
    goal_met: bool,
) -> Result<i32, sqlx::Error> {
    sqlx::query("INSERT INTO streaks (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    let (current, best, last_day, freezes_left, reset_month): (i32, i32, Option<NaiveDate>, i32, Option<NaiveDate>) =
        sqlx::query_as(
            "SELECT current, best, last_day, freezes_left, freezes_reset_month FROM streaks WHERE user_id = $1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await?;
    let before = StreakState {
        current,
        best,
        last_day,
        freezes_left,
        freezes_reset_month: reset_month,
    };
    let pro: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM subscriptions WHERE user_id = $1 AND ends_at > now())",
    )
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await?;
    if !goal_met {
        return Ok(streak::effective(&before, today, pro));
    }
    let after = streak::record_goal_day(
        before,
        today,
        pro,
        state.settings.int("streak_freezes_per_month") as i32,
    );
    sqlx::query(
        "UPDATE streaks SET current = $2, best = $3, last_day = $4, freezes_left = $5, freezes_reset_month = $6
         WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(after.current)
    .bind(after.best)
    .bind(after.last_day)
    .bind(after.freezes_left)
    .bind(after.freezes_reset_month)
    .execute(&mut *conn)
    .await?;
    Ok(after.current)
}

async fn award_badges(
    conn: &mut PgConnection,
    s: &Solved<'_>,
    breakdown: &Breakdown,
    lesson_complete: bool,
    streak_now: i32,
) -> Result<Vec<&'static str>, sqlx::Error> {
    let total_sentences: i64 =
        sqlx::query_scalar("SELECT count(*) FROM progress WHERE user_id = $1")
            .bind(s.user_id)
            .fetch_one(&mut *conn)
            .await?;
    let lessons_completed_total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM lesson_progress WHERE user_id = $1")
            .bind(s.user_id)
            .fetch_one(&mut *conn)
            .await?;
    let (mut lesson_all_clean, mut situation_completed_now) = (false, false);
    if lesson_complete {
        lesson_all_clean = sqlx::query_scalar(
            "SELECT NOT EXISTS (
                SELECT 1 FROM sentences x JOIN progress p ON p.sentence_id = x.id AND p.user_id = $1
                WHERE x.lesson_id = $2 AND x.active AND NOT p.clean)",
        )
        .bind(s.user_id)
        .bind(s.lesson_id)
        .fetch_one(&mut *conn)
        .await?;
        situation_completed_now = sqlx::query_scalar(
            "SELECT NOT EXISTS (
                SELECT 1 FROM lessons l
                WHERE l.situation_id = (SELECT situation_id FROM lessons WHERE id = $2)
                  AND NOT EXISTS (SELECT 1 FROM lesson_progress lp WHERE lp.user_id = $1 AND lp.lesson_id = l.id))",
        )
        .bind(s.user_id)
        .bind(s.lesson_id)
        .fetch_one(&mut *conn)
        .await?;
    }
    let snapshot = Snapshot {
        total_sentences,
        streak: streak_now,
        lesson_completed_now: lesson_complete,
        lessons_completed_total,
        lesson_all_clean,
        situation_completed_now,
        letters: s.letters,
        time_ms: s.elapsed_ms,
        flagged: breakdown.flagged,
    };
    let mut fresh = Vec::new();
    for badge in badges::earned(&snapshot) {
        let inserted = sqlx::query(
            "INSERT INTO user_badges (user_id, badge) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(s.user_id)
        .bind(badge)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        if inserted > 0 {
            fresh.push(badge);
        }
    }
    Ok(fresh)
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
