//! Streaks, leagues, badges, statistics and the scheduled jobs, against a
//! throwaway schema. Skipped unless `TEST_DATABASE_URL` is set.
use super::bot_tests::start_stub;
use super::flow_tests::{Client, init_data, seed};
use super::*;
use crate::{db, jobs};
use axum::http::StatusCode;
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Africa::Cairo;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

fn cairo(y: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<chrono::Utc> {
    Cairo
        .with_ymd_and_hms(y, m, d, h, min, 0)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

async fn state_with(pool: &PgPool, origin: &str, channel: Option<&str>) -> AppState {
    let extra: Vec<(&str, &str)> = channel
        .map(|c| ("CHALLENGE_CHANNEL_ID", c))
        .into_iter()
        .collect();
    state_with_vars(pool, origin, &extra).await
}

pub(super) async fn state_with_vars(
    pool: &PgPool,
    origin: &str,
    extra: &[(&str, &str)],
) -> AppState {
    let mut vars: Vec<(String, String)> = vec![
        ("BOT_TOKEN", "123456:TEST-TOKEN"),
        ("PUBLIC_URL", "https://example.com"),
        ("BIND_ADDR", "127.0.0.1:0"),
        ("DATABASE_URL", "postgresql://x:x@127.0.0.1:1/x"),
        ("WEBHOOK_PATH", "path"),
        ("WEBHOOK_SECRET", "secret"),
        ("ADSGRAM_REWARD_SECRET", "reward"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    vars.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    AppState {
        pool: pool.clone(),
        config: Arc::new(Config::from_vars(vars).unwrap()),
        settings: Arc::new(Settings::load(pool).await.unwrap()),
        bot: Arc::new(Bot::new("123456:TEST-TOKEN", origin).unwrap()),
        texts: Arc::new(Texts::load().unwrap()),
        gemini: None,
    }
}

pub(super) async fn set(pool: &PgPool, state: &AppState, key: &str, value: Value) {
    sqlx::query("UPDATE settings SET value = $2 WHERE key = $1")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
    state.settings.reload(pool).await.unwrap();
}

#[tokio::test]
async fn streak_league_badges_and_stats_through_the_api() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (lesson1, _) = seed(&pool).await;
    let (_, origin) = start_stub().await;
    let state = state_with(&pool, &origin, None).await;
    set(&pool, &state, "streak_goal", json!(2)).await;
    let client = Client {
        app: router(state),
        auth: init_data(11),
    };
    let pause = || tokio::time::sleep(std::time::Duration::from_millis(300));

    // Sentence 1 with a mistake, sentence 2 clean: the lesson completes but is not perfect.
    let (_, a) = client
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    let check = format!("/api/attempts/{}/check", a["attempt_id"]);
    client
        .call("POST", &check, Some(json!({"words": ["i", "am", "fone"]})))
        .await;
    pause().await;
    let (_, r) = client
        .call("POST", &check, Some(json!({"words": ["i", "am", "fine"]})))
        .await;
    assert_eq!(r["solved"], true);
    assert_eq!(r["streak"], 0, "goal of 2 not met yet");
    let (_, me) = client.call("GET", "/api/me", None).await;
    assert_eq!(me["streak"]["current"], 0);

    let (_, a) = client
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    let check = format!("/api/attempts/{}/check", a["attempt_id"]);
    client
        .call("POST", &check, Some(json!({"words": ["good", "morning"]})))
        .await;
    let (_, r) = client
        .call("POST", &check, Some(json!({"words": ["good", "morning"]})))
        .await;
    // The first call may already have solved it; fetch the solved payload either way.
    let solved = if r["solved"] == true { r } else { json!(null) };
    if solved.is_null() {
        pause().await;
    }
    let (_, me) = client.call("GET", "/api/me", None).await;
    assert_eq!(me["streak"]["current"], 1, "goal met: streak starts");

    let (_, stats) = client.call("GET", "/api/stats", None).await;
    assert_eq!(stats["streak"], 1);

    // League: placed in a Bronze group with the points of both sentences.
    let (_, lg) = client.call("GET", "/api/league", None).await;
    assert_eq!(lg["tier"], "bronze");
    assert_eq!(lg["joined"], true);
    assert_eq!(lg["members"].as_array().unwrap().len(), 1);
    assert_eq!(lg["members"][0]["me"], true);
    assert!(lg["members"][0]["points"].as_i64().unwrap() > 0);
    assert_eq!(lg["members"][0]["rank"], 1);

    // Badges: first lesson yes, perfect lesson no (a mistake was made).
    let (_, b) = client.call("GET", "/api/badges", None).await;
    let earned: Vec<&str> = b["earned"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["badge"].as_str().unwrap())
        .collect();
    assert!(earned.contains(&"first_lesson"), "{earned:?}");
    assert!(!earned.contains(&"perfect_lesson"), "{earned:?}");
    assert_eq!(b["all"].as_array().unwrap().len(), 13);

    // Pro statistics: refused for free users, then the mistake shows up.
    let (status, body) = client.call("GET", "/api/stats/pro", None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("pro_required"))
    );
    sqlx::query("INSERT INTO subscriptions (user_id, plan, source, starts_at, ends_at) VALUES (11, 'month', 'manual', now(), now() + interval '30 days')")
        .execute(&pool)
        .await
        .unwrap();
    let (status, pro) = client.call("GET", "/api/stats/pro", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pro["days"].as_array().unwrap().len(), 14);
    assert_eq!(pro["days"][13]["sentences"], 2);
    assert_eq!(pro["letters"], json!([["i", 1]]));
    assert_eq!(pro["words"], json!([["fine", 1]]));

    db::drop_test_schema(&pool, &schema).await;
}

#[tokio::test]
async fn league_group_fills_then_a_new_group_opens() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (lesson1, _) = seed(&pool).await;
    let (_, origin) = start_stub().await;
    let state = state_with(&pool, &origin, None).await;
    set(&pool, &state, "league_group_size", json!(2)).await;
    for user in [21, 22, 23] {
        let client = Client {
            app: router(state.clone()),
            auth: init_data(user),
        };
        let (_, a) = client
            .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
            .await;
        let check = format!("/api/attempts/{}/check", a["attempt_id"]);
        client
            .call("POST", &check, Some(json!({"words": ["i", "am", "fine"]})))
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    }
    let sizes: Vec<i64> =
        sqlx::query_scalar("SELECT count(*) FROM league_members GROUP BY group_id ORDER BY 1 DESC")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(sizes, [2, 1]);
    db::drop_test_schema(&pool, &schema).await;
}

async fn user(pool: &PgPool, id: i64) {
    sqlx::query("INSERT INTO users (id, name) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(id)
        .bind(format!("user{id}"))
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn weekly_job_moves_tiers_once_and_purges_old_state() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (_, origin) = start_stub().await;
    let state = state_with(&pool, &origin, None).await;
    let week = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(); // a Saturday
    sqlx::query("INSERT INTO league_weeks (week_start) VALUES ($1)")
        .bind(week)
        .execute(&pool)
        .await
        .unwrap();
    let group: i64 = sqlx::query_scalar(
        "INSERT INTO league_groups (week_start, tier) VALUES ($1, 2) RETURNING id",
    )
    .bind(week)
    .fetch_one(&pool)
    .await
    .unwrap();
    for i in 1..=20_i64 {
        user(&pool, 100 + i).await;
        sqlx::query("INSERT INTO user_tiers (user_id, tier) VALUES ($1, 2)")
            .bind(100 + i)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO league_members (group_id, week_start, user_id, points) VALUES ($1, $2, $3, $4)")
            .bind(group)
            .bind(week)
            .bind(100 + i)
            .bind((100 - i) as i32)
            .execute(&pool)
            .await
            .unwrap();
    }
    // A finished Bronze group: the bottom stays, the top rises.
    let bronze: i64 = sqlx::query_scalar(
        "INSERT INTO league_groups (week_start, tier) VALUES ($1, 0) RETURNING id",
    )
    .bind(week)
    .fetch_one(&pool)
    .await
    .unwrap();
    for i in 1..=10_i64 {
        user(&pool, 200 + i).await;
        sqlx::query("INSERT INTO user_tiers (user_id, tier) VALUES ($1, 0)")
            .bind(200 + i)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO league_members (group_id, week_start, user_id, points) VALUES ($1, $2, $3, $4)")
            .bind(bronze)
            .bind(week)
            .bind(200 + i)
            .bind((50 - i) as i32)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Still inside the week: nothing happens.
    jobs::run_due(&state, cairo(2026, 10, 16, 12, 0)).await;
    let processed: bool =
        sqlx::query_scalar("SELECT processed FROM league_weeks WHERE week_start = $1")
            .bind(week)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!processed);

    // After the week ends it runs once, and running again moves nobody.
    jobs::run_due(&state, cairo(2026, 10, 17, 0, 5)).await;
    let tiers = |first: i64, last: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (i64, i16)>("SELECT user_id, tier FROM user_tiers WHERE user_id BETWEEN $1 AND $2 ORDER BY user_id")
                .bind(first)
                .bind(last)
                .fetch_all(&pool)
                .await
                .unwrap()
        }
    };
    let gold = tiers(101, 120).await;
    assert_eq!(gold.iter().filter(|(_, t)| *t == 3).count(), 7);
    assert_eq!(gold.iter().filter(|(_, t)| *t == 1).count(), 5);
    assert_eq!(gold.iter().filter(|(_, t)| *t == 2).count(), 8);
    let bronze_after = tiers(201, 210).await;
    assert_eq!(bronze_after.iter().filter(|(_, t)| *t == 1).count(), 7);
    assert_eq!(bronze_after.iter().filter(|(_, t)| *t == 0).count(), 3);
    assert_eq!(jobs::process_week(&state, week).await.unwrap(), 0);
    assert_eq!(tiers(101, 120).await, gold);
    let promoted: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_badges WHERE badge = 'first_promotion'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(promoted, 14);

    // Purge: day-old attempts, update ids and expired ad credits go; fresh ones stay.
    sqlx::query("INSERT INTO situations (slug, name_ar, name_en, icon, position) VALUES ('x','x','x','x',0)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO lessons (situation_id, number) SELECT id, 1 FROM situations")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sentences (lesson_id, position, text, normalized, translation, difficulty, tokens) SELECT id, 1, 'a', 'a', 'a', 1, '[]' FROM lessons").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attempts (user_id, sentence_id, kind, started_at) SELECT 101, (SELECT id FROM sentences), 'lesson', now() - interval '25 hours'").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attempts (user_id, sentence_id, kind) SELECT 101, (SELECT id FROM sentences), 'lesson'").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO processed_updates (update_id, created_at) VALUES (1, now() - interval '25 hours'), (2, now())").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO ad_credits (user_id, created_at) VALUES (101, now() - interval '11 minutes'), (101, now())").execute(&pool).await.unwrap();
    jobs::purge(&pool, 10).await.unwrap();
    let left = |table: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table}"
            )))
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(
        (
            left("attempts").await,
            left("processed_updates").await,
            left("ad_credits").await
        ),
        (1, 1, 1)
    );

    db::drop_test_schema(&pool, &schema).await;
}

#[tokio::test]
async fn daily_challenge_is_picked_once_and_not_repeated() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    seed(&pool).await;
    let (_, origin) = start_stub().await;
    let state = state_with(&pool, &origin, None).await;
    let day = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    let first = crate::challenge::ensure(&pool, &state.settings, day)
        .await
        .unwrap();
    assert_eq!(first.len(), 3); // the test bank only has three sentences
    assert_eq!(
        crate::challenge::ensure(&pool, &state.settings, day)
            .await
            .unwrap(),
        first
    );
    // Everything was used yesterday, so today's pick finds nothing new.
    let next = crate::challenge::ensure(&pool, &state.settings, day.succ_opt().unwrap())
        .await
        .unwrap();
    assert!(next.is_empty());
    db::drop_test_schema(&pool, &schema).await;
}

#[tokio::test]
async fn reminders_send_once_to_the_right_people_and_mark_blocked_users() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (stub, origin) = start_stub().await;
    let state = state_with(&pool, &origin, None).await;
    let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    let yesterday = today.pred_opt().unwrap();
    let make = |id: i64, streak_day: NaiveDate, done_today: i32, reminders: bool| {
        let pool = pool.clone();
        async move {
            user(&pool, id).await;
            sqlx::query("UPDATE users SET reminders_on = $2 WHERE id = $1")
                .bind(id)
                .bind(reminders)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO streaks (user_id, current, best, last_day) VALUES ($1, 3, 3, $2)",
            )
            .bind(id)
            .bind(streak_day)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO daily_stats (user_id, cairo_date, sentences) VALUES ($1, $2, $3)",
            )
            .bind(id)
            .bind(today)
            .bind(done_today)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    make(1, yesterday, 4, true).await; // streak alive, goal not met: reminded
    make(2, yesterday, 10, true).await; // goal met
    make(3, yesterday, 0, false).await; // reminders off
    make(4, today.pred_opt().unwrap().pred_opt().unwrap(), 0, true).await; // streak already lost
    let send_at = cairo(2026, 10, 19, 20, 0);

    // Off by default: nothing is sent until the wording is approved.
    jobs::run_due(&state, send_at).await;
    assert!(stub.methods().is_empty());

    set(&pool, &state, "reminders_enabled", json!(true)).await;
    jobs::run_due(&state, cairo(2026, 10, 19, 19, 59)).await;
    assert!(stub.methods().is_empty(), "wrong hour");
    jobs::run_due(&state, send_at).await;
    assert_eq!(stub.methods(), ["sendMessage"]);
    let sent = stub.last("sendMessage");
    assert_eq!(sent["chat_id"], 1);
    assert!(sent["text"].as_str().unwrap().contains("6")); // 10 - 4 sentences left
    assert_eq!(
        sent["reply_markup"]["inline_keyboard"][1][0]["callback_data"],
        "rem:off"
    );
    jobs::run_due(&state, cairo(2026, 10, 19, 20, 30)).await;
    assert_eq!(stub.methods().len(), 1, "never twice in a day");

    // A user who blocked the bot is flagged and skipped from then on.
    make(5, yesterday, 0, true).await;
    stub.fail_with.store(403, Ordering::SeqCst);
    jobs::run_due(&state, cairo(2026, 10, 19, 20, 40)).await;
    let blocked: bool = sqlx::query_scalar("SELECT bot_blocked FROM users WHERE id = 5")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(blocked);
    stub.fail_with.store(0, Ordering::SeqCst);
    let before = stub.methods().len();
    jobs::run_due(&state, cairo(2026, 10, 19, 20, 50)).await;
    assert_eq!(stub.methods().len(), before);

    // Another failure is retried: the log row is released.
    make(6, yesterday, 0, true).await;
    stub.fail_with.store(500, Ordering::SeqCst);
    jobs::run_due(&state, cairo(2026, 10, 19, 20, 55)).await;
    stub.fail_with.store(0, Ordering::SeqCst);
    let before = stub.methods().len();
    jobs::run_due(&state, cairo(2026, 10, 19, 20, 56)).await;
    assert_eq!(stub.methods().len(), before + 1);

    db::drop_test_schema(&pool, &schema).await;
}

#[tokio::test]
async fn challenge_summary_posts_once_after_the_set_time() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (stub, origin) = start_stub().await;
    let state = state_with(&pool, &origin, Some("-100123")).await;
    let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    sqlx::query("INSERT INTO daily_challenges (cairo_date, sentence_ids) VALUES ($1, '{}')")
        .bind(today)
        .execute(&pool)
        .await
        .unwrap();
    for (id, score) in [(1_i64, 90), (2, 120)] {
        user(&pool, id).await;
        sqlx::query("INSERT INTO challenge_scores (cairo_date, user_id, score, solved) VALUES ($1, $2, $3, 5)").bind(today).bind(id).bind(score).execute(&pool).await.unwrap();
    }

    jobs::run_due(&state, cairo(2026, 10, 19, 23, 40)).await;
    assert!(stub.methods().is_empty(), "disabled until approved");

    set(&pool, &state, "challenge_post_enabled", json!(true)).await;
    jobs::run_due(&state, cairo(2026, 10, 19, 23, 29)).await;
    assert!(stub.methods().is_empty(), "before the post time");

    stub.fail_with.store(500, Ordering::SeqCst);
    jobs::run_due(&state, cairo(2026, 10, 19, 23, 30)).await;
    stub.fail_with.store(0, Ordering::SeqCst);
    let posted: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT posted_at FROM daily_challenges WHERE cairo_date = $1")
            .bind(today)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(posted.is_none(), "a failed send releases the claim");

    jobs::run_due(&state, cairo(2026, 10, 19, 23, 31)).await;
    let sent = stub.last("sendMessage");
    assert_eq!(sent["chat_id"], -100123);
    let text = sent["text"].as_str().unwrap();
    assert!(text.starts_with("⌯"));
    assert!(text.contains("user2 ↢ 120"));
    assert!(
        text.find("user2").unwrap() < text.find("user1").unwrap(),
        "ranked by score"
    );
    jobs::run_due(&state, cairo(2026, 10, 19, 23, 45)).await;
    assert_eq!(
        stub.methods()
            .iter()
            .filter(|m| *m == "sendMessage")
            .count(),
        2,
        "one failed try and one post"
    );

    db::drop_test_schema(&pool, &schema).await;
}
