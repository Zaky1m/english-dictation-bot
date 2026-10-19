use sqlx::PgPool;

pub struct Profile {
    pub lang: String,
    pub blocked: bool,
}

/// Creates the user on first contact and keeps the display name fresh.
pub async fn upsert(
    pool: &PgPool,
    id: i64,
    name: &str,
    username: Option<&str>,
) -> Result<Profile, sqlx::Error> {
    let (lang, blocked): (String, bool) = sqlx::query_as(
        "INSERT INTO users (id, name, username) VALUES ($1, $2, $3)
         ON CONFLICT (id) DO UPDATE SET name = $2, username = $3
         RETURNING lang, blocked",
    )
    .bind(id)
    .bind(name)
    .bind(username)
    .fetch_one(pool)
    .await?;
    Ok(Profile { lang, blocked })
}
