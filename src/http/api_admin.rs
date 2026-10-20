use super::AppState;
use super::error::{ApiError, ApiResult};
use super::user::User;
use crate::cairo;
use crate::content::runner::{self, Kind, StartError};
use crate::telegram::payments::extend_days;
use axum::Json;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A user whose id is in `ADMIN_IDS`. Everything under `/api/admin` needs it.
pub struct Admin(pub User);

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let user = User::from_request_parts(parts, state).await?;
        if state.config.admin_ids.contains(&user.id) {
            Ok(Self(user))
        } else {
            Err(ApiError::Forbidden("admin_only"))
        }
    }
}

pub async fn overview(State(state): State<AppState>, _admin: Admin) -> ApiResult<Value> {
    let today = cairo::date(Utc::now());
    let count = |sql: &'static str| {
        let pool = state.pool.clone();
        async move { sqlx::query_scalar::<_, i64>(sql).fetch_one(&pool).await }
    };
    let users = count("SELECT count(*) FROM users").await?;
    let new_week =
        count("SELECT count(*) FROM users WHERE created_at > now() - interval '7 days'").await?;
    let pro =
        count("SELECT count(DISTINCT user_id) FROM subscriptions WHERE ends_at > now()").await?;
    let sentences_total = count("SELECT count(*) FROM progress").await?;
    let (active_today, sentences_today): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE sentences > 0), COALESCE(sum(sentences), 0)::bigint FROM daily_stats WHERE cairo_date = $1",
    )
    .bind(today)
    .fetch_one(&state.pool)
    .await?;
    let stars_total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM payments WHERE currency = 'XTR'",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({
        "users": users, "new_users_7d": new_week, "active_today": active_today, "pro": pro,
        "sentences_today": sentences_today, "sentences_total": sentences_total, "stars_total": stars_total,
    })))
}

#[derive(Deserialize)]
pub struct UserQuery {
    q: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct UserRow {
    id: i64,
    name: String,
    username: Option<String>,
    pro_until: Option<DateTime<Utc>>,
    blocked: bool,
}

pub async fn users(
    State(state): State<AppState>,
    _admin: Admin,
    Query(query): Query<UserQuery>,
) -> ApiResult<Vec<UserRow>> {
    let text = query.q.unwrap_or_default();
    let text = text.trim().trim_start_matches('@');
    let id: Option<i64> = text.parse().ok();
    let pattern = format!("%{}%", text.replace('%', "\\%").replace('_', "\\_"));
    let rows = sqlx::query_as(
        "SELECT u.id, left(u.name, 40) AS name, u.username, u.blocked,
                (SELECT max(ends_at) FROM subscriptions s WHERE s.user_id = u.id AND s.ends_at > now()) AS pro_until
         FROM users u
         WHERE $1::bigint IS NOT NULL AND u.id = $1 OR u.username ILIKE $2 OR u.name ILIKE $2
         ORDER BY u.created_at DESC LIMIT 20",
    )
    .bind(id)
    .bind(pattern)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct GrantBody {
    plan: String,
    days: Option<i64>,
}

/// Manual Pro grant for payments made outside Telegram Stars. Extends an active subscription.
pub async fn grant(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(user_id): Path<i64>,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, ApiError> {
    let (plan, days) = match body.plan.as_str() {
        "month" => ("month", 30),
        "year" => ("year", 365),
        "custom" => (
            "custom",
            body.days
                .filter(|d| (1..=3650).contains(d))
                .ok_or(ApiError::BadRequest("bad_days"))?,
        ),
        _ => return Err(ApiError::BadRequest("bad_plan")),
    };
    let mut tx = state.pool.begin().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    let current: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT max(ends_at) FROM subscriptions WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
    let (starts, ends) = extend_days(Utc::now(), current, days);
    sqlx::query(
        "INSERT INTO subscriptions (user_id, plan, source, starts_at, ends_at, granted_by) VALUES ($1, $2, 'manual', $3, $4, $5)",
    )
    .bind(user_id)
    .bind(plan)
    .bind(starts)
    .bind(ends)
    .bind(admin.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(
        admin = admin.id,
        user = user_id,
        plan,
        days,
        "manual Pro grant"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// Ends every active subscription now. A recurring Stars subscription is cancelled
/// separately in Telegram; this only removes the access.
pub async fn revoke(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(user_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("UPDATE subscriptions SET starts_at = LEAST(starts_at, now()), ends_at = now() WHERE user_id = $1 AND ends_at > now()")
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    tracing::info!(admin = admin.id, user = user_id, "Pro revoked");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
struct PaymentRow {
    charge_id: String,
    user_id: i64,
    name: String,
    amount: i32,
    currency: String,
    payload: String,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct GrantRow {
    user_id: i64,
    name: String,
    plan: String,
    granted_by: Option<i64>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

pub async fn payments(State(state): State<AppState>, _admin: Admin) -> ApiResult<Value> {
    let payments: Vec<PaymentRow> = sqlx::query_as(
        "SELECT p.telegram_payment_charge_id AS charge_id, p.user_id, left(u.name, 40) AS name, p.amount, p.currency, p.payload, p.created_at
         FROM payments p JOIN users u ON u.id = p.user_id ORDER BY p.created_at DESC LIMIT 50",
    )
    .fetch_all(&state.pool)
    .await?;
    let grants: Vec<GrantRow> = sqlx::query_as(
        "SELECT s.user_id, left(u.name, 40) AS name, s.plan, s.granted_by, s.starts_at, s.ends_at
         FROM subscriptions s JOIN users u ON u.id = s.user_id WHERE s.source = 'manual' ORDER BY s.id DESC LIMIT 30",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "payments": payments, "grants": grants })))
}

pub async fn settings(
    State(state): State<AppState>,
    _admin: Admin,
) -> ApiResult<Vec<(String, Value)>> {
    let rows = sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct SettingBody {
    value: Value,
}

fn same_kind(old: &Value, new: &Value) -> bool {
    match (old, new) {
        (Value::Number(_), Value::Number(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::String(_), Value::String(_)) => true,
        // A list keeps the kind of its items: numbers stay numbers, names stay names.
        (Value::Array(before), Value::Array(items)) => {
            if before.iter().any(Value::is_string) {
                items.iter().all(Value::is_string)
            } else {
                items.iter().all(Value::is_number)
            }
        }
        _ => false,
    }
}

/// Saves one setting and reloads the cache, so the change applies immediately.
pub async fn save_setting(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(key): Path<String>,
    Json(body): Json<SettingBody>,
) -> Result<StatusCode, ApiError> {
    let old: Value = sqlx::query_scalar("SELECT value FROM settings WHERE key = $1")
        .bind(&key)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    if !same_kind(&old, &body.value) {
        return Err(ApiError::BadRequest("wrong_type"));
    }
    state.settings.save(&state.pool, &key, &body.value).await?;
    tracing::info!(admin = admin.id, key, "setting changed");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SituationRow {
    id: i32,
    slug: String,
    name_ar: String,
    name_en: String,
    icon: String,
    active: bool,
    sentences: i64,
    audio_missing: i64,
}

pub async fn situations(
    State(state): State<AppState>,
    _admin: Admin,
) -> ApiResult<Vec<SituationRow>> {
    let rows = sqlx::query_as(
        "SELECT s.id, s.slug, s.name_ar, s.name_en, s.icon, s.active,
                count(x.id) FILTER (WHERE x.active) AS sentences,
                count(x.id) FILTER (WHERE x.active AND NOT x.audio_ready) AS audio_missing
         FROM situations s LEFT JOIN lessons l ON l.situation_id = s.id LEFT JOIN sentences x ON x.lesson_id = l.id
         GROUP BY s.id ORDER BY s.position",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SentenceRow {
    id: i32,
    lesson: i16,
    position: i16,
    text: String,
    translation: String,
    active: bool,
    audio_ready: bool,
}

pub async fn situation_sentences(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i32>,
) -> ApiResult<Vec<SentenceRow>> {
    let rows = sqlx::query_as(
        "SELECT x.id, l.number AS lesson, x.position, x.text, x.translation, x.active, x.audio_ready
         FROM sentences x JOIN lessons l ON l.id = x.lesson_id WHERE l.situation_id = $1 ORDER BY l.number, x.position",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct NewSituation {
    slug: String,
    name_ar: String,
    name_en: String,
    icon: String,
}

fn start_error(e: StartError) -> ApiError {
    match e {
        StartError::NoGemini => ApiError::Forbidden("gemini_unavailable"),
        StartError::Busy => ApiError::TooManyRequests,
        StartError::Db(message) => ApiError::Internal(anyhow::anyhow!(message)),
    }
}

/// Creates an unpublished situation with six empty lessons and starts writing it.
/// It stays hidden until the admin reviews the sentences and publishes it.
pub async fn create_situation(
    State(state): State<AppState>,
    _admin: Admin,
    Json(body): Json<NewSituation>,
) -> ApiResult<Value> {
    let slug_ok = (2..=40).contains(&body.slug.len())
        && body
            .slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !slug_ok
        || body.name_ar.trim().is_empty()
        || body.name_en.trim().is_empty()
        || body.icon.trim().is_empty()
    {
        return Err(ApiError::BadRequest("bad_situation"));
    }
    if state.gemini.is_none() {
        return Err(ApiError::Forbidden("gemini_unavailable"));
    }
    let mut tx = state.pool.begin().await?;
    let id: Option<i32> = sqlx::query_scalar(
        "INSERT INTO situations (slug, name_ar, name_en, icon, position, active)
         VALUES ($1, $2, $3, $4, (SELECT COALESCE(max(position), -1) + 1 FROM situations), false)
         ON CONFLICT (slug) DO NOTHING RETURNING id",
    )
    .bind(&body.slug)
    .bind(body.name_ar.trim())
    .bind(body.name_en.trim())
    .bind(body.icon.trim())
    .fetch_optional(&mut *tx)
    .await?;
    let id = id.ok_or(ApiError::BadRequest("slug_taken"))?;
    for number in 1..=6_i16 {
        sqlx::query("INSERT INTO lessons (situation_id, number) VALUES ($1, $2)")
            .bind(id)
            .bind(number)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let job = runner::start(&state, Kind::Generate(id))
        .await
        .map_err(start_error)?;
    Ok(Json(json!({ "id": id, "job": job })))
}

/// Resumes writing a situation (for example after a failure): only missing sentences are generated.
pub async fn generate_situation(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i32>,
) -> ApiResult<Value> {
    let job = runner::start(&state, Kind::Generate(id))
        .await
        .map_err(start_error)?;
    Ok(Json(json!({ "job": job })))
}

#[derive(Deserialize)]
pub struct ActiveBody {
    active: bool,
}

pub async fn publish_situation(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i32>,
    Json(body): Json<ActiveBody>,
) -> Result<StatusCode, ApiError> {
    if body.active {
        // A half-written situation would show empty lessons on the map.
        let incomplete: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM lessons l WHERE l.situation_id = $1
               AND (SELECT count(*) FROM sentences x WHERE x.lesson_id = l.id AND x.active) = 0)",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
        if incomplete {
            return Err(ApiError::BadRequest("incomplete"));
        }
    }
    let changed = sqlx::query("UPDATE situations SET active = $2 WHERE id = $1")
        .bind(id)
        .bind(body.active)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_sentence_active(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i32>,
    Json(body): Json<ActiveBody>,
) -> Result<StatusCode, ApiError> {
    let changed = sqlx::query("UPDATE sentences SET active = $2 WHERE id = $1")
        .bind(id)
        .bind(body.active)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Marks the situation's audio (sentences and their words) as missing and rebuilds it.
pub async fn regenerate_situation_audio(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i32>,
) -> ApiResult<Value> {
    if state.gemini.is_none() {
        return Err(ApiError::Forbidden("gemini_unavailable"));
    }
    sqlx::query(
        "UPDATE sentences SET audio_ready = false WHERE lesson_id IN (SELECT id FROM lessons WHERE situation_id = $1)",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE words SET audio_ready = false WHERE word IN (
            SELECT sw.word FROM sentence_words sw JOIN sentences x ON x.id = sw.sentence_id
            JOIN lessons l ON l.id = x.lesson_id WHERE l.situation_id = $1)",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    let job = runner::start(&state, Kind::Audio(Some(id)))
        .await
        .map_err(start_error)?;
    Ok(Json(json!({ "job": job })))
}

pub async fn regenerate_sentence_audio(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i32>,
) -> ApiResult<Value> {
    if state.gemini.is_none() {
        return Err(ApiError::Forbidden("gemini_unavailable"));
    }
    sqlx::query("UPDATE sentences SET audio_ready = false WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    let job = runner::start(&state, Kind::Audio(None))
        .await
        .map_err(start_error)?;
    Ok(Json(json!({ "job": job })))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobRow {
    id: i64,
    kind: String,
    situation_id: Option<i32>,
    status: String,
    detail: String,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

pub async fn jobs(State(state): State<AppState>, _admin: Admin) -> ApiResult<Vec<JobRow>> {
    let rows = sqlx::query_as("SELECT id, kind, situation_id, status, detail, created_at, finished_at FROM content_jobs ORDER BY id DESC LIMIT 20")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows))
}

#[cfg(test)]
mod setting_kind_tests {
    use super::same_kind;
    use serde_json::json;

    #[test]
    fn lists_keep_the_kind_of_their_items() {
        assert!(same_kind(&json!([1, 3, 7]), &json!([2, 4])));
        assert!(!same_kind(&json!([1, 3, 7]), &json!(["a"])));
        assert!(same_kind(
            &json!(["adsgram", "monetag"]),
            &json!(["monetag"])
        ));
        assert!(!same_kind(&json!(["adsgram"]), &json!([1])));
        assert!(!same_kind(&json!(true), &json!(1)));
    }
}
