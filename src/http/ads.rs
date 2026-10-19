use sqlx::PgConnection;

/// Consumes one unclaimed, unexpired ad credit for `purpose` ('reveal' or
/// 'extra'). The row lock plus `SKIP LOCKED` makes two simultaneous requests
/// unable to spend the same credit.
pub async fn consume_credit(
    conn: &mut PgConnection,
    user_id: i64,
    ttl_minutes: i64,
    purpose: &str,
) -> Result<bool, sqlx::Error> {
    let id: Option<i64> = sqlx::query_scalar(
        "UPDATE ad_credits SET claimed_at = now(), claimed_for = $3
         WHERE id = (SELECT id FROM ad_credits
                     WHERE user_id = $1 AND claimed_at IS NULL
                       AND created_at > now() - make_interval(mins => $2)
                     ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED)
         RETURNING id",
    )
    .bind(user_id)
    .bind(ttl_minutes as i32)
    .bind(purpose)
    .fetch_optional(conn)
    .await?;
    Ok(id.is_some())
}

/// The rewarded-ad networks that have their ids set, in the admin's preferred
/// order. The Mini App tries them one after another until one shows an ad.
pub fn providers(state: &super::AppState) -> Vec<serde_json::Value> {
    use serde_json::json;
    let c = &state.config;
    let mut order = state.settings.text_list("ad_provider_order");
    for name in ALL_PROVIDERS {
        if !order.iter().any(|o| o == name) {
            order.push((*name).to_owned());
        }
    }
    order
        .iter()
        .filter_map(|name| match name.as_str() {
            "adsgram" => c
                .adsgram_block_id
                .as_ref()
                .map(|id| json!({ "name": name, "block_id": id })),
            "monetag" => c
                .monetag_zone_id
                .as_ref()
                .map(|id| json!({ "name": name, "zone_id": id })),
            "adexium" => c
                .adexium_widget_id
                .as_ref()
                .map(|id| json!({ "name": name, "widget_id": id })),
            "telega" => c
                .telega_token
                .as_ref()
                .zip(c.telega_block_uuid.as_ref())
                .map(|(token, block)| json!({ "name": name, "token": token, "block_uuid": block })),
            "gigapub" => c
                .gigapub_project_id
                .as_ref()
                .map(|id| json!({ "name": name, "project_id": id })),
            _ => None,
        })
        .collect()
}

pub const ALL_PROVIDERS: &[&str] = &["adsgram", "monetag", "adexium", "telega", "gigapub"];

/// Adds one unclaimed credit, unless the user already holds the maximum or got
/// one moments ago. Returns false when the credit was refused.
pub async fn add_credit(
    conn: &mut PgConnection,
    user_id: i64,
    ttl_minutes: i64,
) -> Result<bool, sqlx::Error> {
    const MAX_OPEN_CREDITS: i64 = 3;
    const MIN_GAP_SECS: i32 = 20;
    let added = sqlx::query(
        "INSERT INTO ad_credits (user_id)
         SELECT $1 WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)
           AND (SELECT count(*) FROM ad_credits WHERE user_id = $1 AND claimed_at IS NULL
                  AND created_at > now() - make_interval(mins => $2)) < $3
           AND NOT EXISTS (SELECT 1 FROM ad_credits WHERE user_id = $1
                  AND created_at > now() - make_interval(secs => $4))",
    )
    .bind(user_id)
    .bind(ttl_minutes as i32)
    .bind(MAX_OPEN_CREDITS)
    .bind(MIN_GAP_SECS)
    .execute(conn)
    .await?
    .rows_affected();
    Ok(added > 0)
}
