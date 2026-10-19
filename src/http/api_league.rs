use super::AppState;
use super::error::{ApiError, ApiResult};
use super::user::{User, is_pro};
use crate::badges;
use crate::cairo;
use crate::league::{self, TIERS};
use axum::Json;
use axum::extract::State;
use chrono::{Duration, NaiveDate, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct LeagueView {
    tier: &'static str,
    tier_index: i16,
    week_start: NaiveDate,
    week_end: NaiveDate,
    promote_top: i64,
    demote_bottom: i64,
    /// False until the user earns their first points this week.
    joined: bool,
    members: Vec<Member>,
}

#[derive(Serialize, sqlx::FromRow)]
struct Member {
    #[sqlx(skip)]
    rank: usize,
    name: String,
    points: i32,
    pro: bool,
    me: bool,
}

pub async fn current(State(state): State<AppState>, user: User) -> ApiResult<LeagueView> {
    let week = league::week_start(cairo::date(Utc::now()));
    let tier: i16 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT tier FROM user_tiers WHERE user_id = $1), 0)::smallint",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let group: Option<i64> = sqlx::query_scalar(
        "SELECT group_id FROM league_members WHERE week_start = $1 AND user_id = $2",
    )
    .bind(week)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    let mut members: Vec<Member> = match group {
        Some(group) => sqlx::query_as(
            "SELECT left(u.name, 24) AS name, m.points,
                    EXISTS (SELECT 1 FROM subscriptions s WHERE s.user_id = u.id AND s.ends_at > now()) AS pro,
                    (u.id = $2) AS me
             FROM league_members m JOIN users u ON u.id = m.user_id
             WHERE m.group_id = $1 ORDER BY m.points DESC, m.user_id",
        )
        .bind(group)
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?,
        None => Vec::new(),
    };
    for (i, m) in members.iter_mut().enumerate() {
        m.rank = i + 1;
    }
    Ok(Json(LeagueView {
        tier: TIERS[tier.clamp(0, league::TOP_TIER) as usize],
        tier_index: tier,
        week_start: week,
        week_end: week + Duration::days(7),
        promote_top: state.settings.int("league_promote_top"),
        demote_bottom: state.settings.int("league_demote_bottom"),
        joined: group.is_some(),
        members,
    }))
}

#[derive(Serialize)]
pub struct BadgesView {
    all: [&'static str; 13],
    earned: Vec<EarnedBadge>,
}

#[derive(Serialize, sqlx::FromRow)]
struct EarnedBadge {
    badge: String,
    earned_at: chrono::DateTime<Utc>,
}

pub async fn badges(State(state): State<AppState>, user: User) -> ApiResult<BadgesView> {
    let earned = sqlx::query_as(
        "SELECT badge, earned_at FROM user_badges WHERE user_id = $1 ORDER BY earned_at",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(BadgesView {
        all: badges::ALL,
        earned,
    }))
}

#[derive(Serialize)]
pub struct ProStats {
    days: Vec<Day>,
    letters: Vec<(String, i32)>,
    words: Vec<(String, i32)>,
}

#[derive(Serialize)]
struct Day {
    date: NaiveDate,
    sentences: i32,
    points: i32,
}

/// Detailed statistics, a Pro feature: the last 14 days and the most missed letters and words.
pub async fn pro_stats(State(state): State<AppState>, user: User) -> ApiResult<ProStats> {
    if !is_pro(&state.pool, user.id).await? {
        return Err(ApiError::Forbidden("pro_required"));
    }
    let today = cairo::date(Utc::now());
    let rows: Vec<(NaiveDate, i32, i32)> = sqlx::query_as(
        "SELECT cairo_date, sentences, points FROM daily_stats WHERE user_id = $1 AND cairo_date > $2",
    )
    .bind(user.id)
    .bind(today - Duration::days(14))
    .fetch_all(&state.pool)
    .await?;
    let days = (0..14)
        .rev()
        .map(|back| {
            let date = today - Duration::days(back);
            let (sentences, points) = rows
                .iter()
                .find(|r| r.0 == date)
                .map_or((0, 0), |r| (r.1, r.2));
            Day {
                date,
                sentences,
                points,
            }
        })
        .collect();
    let letters: Vec<(String, i32)> = sqlx::query_as(
        "SELECT letter::text, count FROM missed_letters WHERE user_id = $1 ORDER BY count DESC, letter LIMIT 6",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let words: Vec<(String, i32)> = sqlx::query_as(
        "SELECT word, count FROM missed_words WHERE user_id = $1 ORDER BY count DESC, word LIMIT 6",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(ProStats {
        days,
        letters,
        words,
    }))
}
