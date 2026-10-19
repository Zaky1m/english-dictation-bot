use super::AppState;
use super::error::ApiResult;
use super::user::{User, is_pro};
use crate::cairo;
use axum::Json;
use axum::extract::State;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Serialize)]
pub struct Me {
    id: i64,
    lang: String,
    pro: bool,
    ads_enabled: bool,
    usage: Usage,
    streak: Streak,
}

#[derive(Serialize)]
struct Usage {
    started: i64,
    limit: i64,
    ad_unlocks_left: i64,
}

#[derive(Serialize)]
struct Streak {
    current: i64,
    best: i64,
}

/// Sentences a user may start today: Pro has a higher base and no ad bonus.
pub fn daily_base_limit(state: &AppState, pro: bool) -> i64 {
    if pro {
        state.settings.int("pro_daily_sentences")
    } else {
        state.settings.int("free_daily_sentences")
    }
}

/// Ads show only when switched on and an Adsgram block is configured.
pub fn ads_on(state: &AppState) -> bool {
    state.settings.bool("ads_enabled") && state.config.adsgram_block_id.is_some()
}

pub async fn me(State(state): State<AppState>, user: User) -> ApiResult<Me> {
    let pro = is_pro(&state.pool, user.id).await?;
    let today = cairo::date(chrono::Utc::now());
    let (started, ad_unlocks): (i32, i32) = sqlx::query_as(
        "SELECT started, ad_unlocks FROM daily_usage WHERE user_id = $1 AND cairo_date = $2",
    )
    .bind(user.id)
    .bind(today)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or((0, 0));
    let bonus = if pro {
        0
    } else {
        i64::from(ad_unlocks) * state.settings.int("ad_extra_sentences")
    };
    let (current, best): (i32, i32) =
        sqlx::query_as("SELECT current, best FROM streaks WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(&state.pool)
            .await?
            .unwrap_or((0, 0));
    let ad_unlocks_left = if pro || !ads_on(&state) {
        0
    } else {
        (state.settings.int("ad_unlocks_per_day") - i64::from(ad_unlocks)).max(0)
    };
    Ok(Json(Me {
        id: user.id,
        lang: user.lang,
        pro,
        ads_enabled: ads_on(&state),
        usage: Usage {
            started: i64::from(started),
            limit: daily_base_limit(&state, pro) + bonus,
            ad_unlocks_left,
        },
        streak: Streak {
            current: i64::from(current),
            best: i64::from(best),
        },
    }))
}

#[derive(FromRow)]
struct MapRow {
    situation_id: i32,
    slug: String,
    name_ar: String,
    name_en: String,
    icon: String,
    lesson_id: i32,
    number: i16,
    total: i64,
    done: i64,
    completed: bool,
}

#[derive(Serialize)]
pub struct MapSituation {
    id: i32,
    slug: String,
    name_ar: String,
    name_en: String,
    icon: String,
    lessons: Vec<MapLesson>,
}

#[derive(Serialize)]
struct MapLesson {
    id: i32,
    number: i16,
    state: &'static str,
    done: i64,
    total: i64,
}

pub async fn map(State(state): State<AppState>, user: User) -> ApiResult<Vec<MapSituation>> {
    let pro = is_pro(&state.pool, user.id).await?;
    let rows: Vec<MapRow> = sqlx::query_as(
        "SELECT s.id AS situation_id, s.slug, s.name_ar, s.name_en, s.icon,
                l.id AS lesson_id, l.number,
                (SELECT count(*) FROM sentences x WHERE x.lesson_id = l.id AND x.active) AS total,
                (SELECT count(*) FROM progress p JOIN sentences x ON x.id = p.sentence_id
                  WHERE p.user_id = $1 AND x.lesson_id = l.id AND x.active) AS done,
                EXISTS (SELECT 1 FROM lesson_progress lp WHERE lp.user_id = $1 AND lp.lesson_id = l.id) AS completed
         FROM situations s JOIN lessons l ON l.situation_id = s.id
         WHERE s.active ORDER BY s.position, l.number",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let mut result: Vec<MapSituation> = Vec::new();
    // A lesson opens when the one before it, across situations, is complete.
    let mut previous_completed = true;
    for row in rows {
        let state_name = if row.completed {
            "done"
        } else if pro || previous_completed {
            "open"
        } else {
            "locked"
        };
        previous_completed = row.completed;
        if result.last().is_none_or(|s| s.id != row.situation_id) {
            result.push(MapSituation {
                id: row.situation_id,
                slug: row.slug,
                name_ar: row.name_ar,
                name_en: row.name_en,
                icon: row.icon,
                lessons: Vec::new(),
            });
        }
        if let Some(situation) = result.last_mut() {
            situation.lessons.push(MapLesson {
                id: row.lesson_id,
                number: row.number,
                state: state_name,
                done: row.done,
                total: row.total,
            });
        }
    }
    Ok(Json(result))
}
