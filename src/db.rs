use anyhow::Context;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

// Small pool: the host is shared and RAM is tight.
const MAX_CONNECTIONS: u32 = 5;

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect(database_url)
        .await
        .context("cannot connect to PostgreSQL")
}

pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("migration failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs only when TEST_DATABASE_URL points at a disposable database.
    #[tokio::test]
    async fn migrations_apply_and_seed_settings() {
        let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
            return;
        };
        let pool = connect(&url).await.unwrap();
        migrate(&pool).await.unwrap();
        migrate(&pool).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(count >= 30);
    }
}
