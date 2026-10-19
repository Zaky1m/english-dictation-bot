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

/// A throwaway schema (the name is generated here from random hex, never user input) inside the database named by `TEST_DATABASE_URL`, fully
/// migrated, so tests never touch real tables. `None` when the variable is unset.
#[cfg(test)]
pub async fn test_pool() -> Option<(PgPool, String)> {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let schema = format!("t_{:016x}", rand::random::<u64>());
    let admin = connect(&url).await.ok()?;
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .ok()?;
    admin.close().await;
    let options = PgConnectOptions::from_str(&url)
        .ok()?
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(options)
        .await
        .ok()?;
    migrate(&pool).await.ok()?;
    Some((pool, schema))
}

#[cfg(test)]
pub async fn drop_test_schema(pool: &PgPool, schema: &str) {
    let _ = sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(pool)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_and_seed_settings() {
        let Some((pool, schema)) = test_pool().await else {
            return;
        };
        migrate(&pool).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(count >= 30);
        drop_test_schema(&pool, &schema).await;
    }
}
