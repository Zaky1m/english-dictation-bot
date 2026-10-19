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
