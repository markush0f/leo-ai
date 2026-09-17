use sqlx::PgPool;

const SCHEMA: &str = include_str!("../../../deploy/postgres/init.sql");
const MIGRATION_001: &str =
    include_str!("../../../deploy/postgres/migrations/001_engines_conversations.sql");
const MIGRATION_002: &str =
    include_str!("../../../deploy/postgres/migrations/002_codex_provider.sql");

const MIGRATIONS: &[(i32, &str)] = &[(1, MIGRATION_001), (2, MIGRATION_002)];

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(pool)
    .await?;

    let providers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name = 'providers'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if providers == 0 {
        for stmt in statements(SCHEMA) {
            sqlx::query(stmt).execute(pool).await?;
        }
        sqlx::query("INSERT INTO schema_migrations (version) VALUES (1) ON CONFLICT DO NOTHING")
            .execute(pool)
            .await?;
        return Ok(());
    }

    let applied: Vec<i32> = sqlx::query_scalar("SELECT version FROM schema_migrations")
        .fetch_all(pool)
        .await?;

    for &(version, sql) in MIGRATIONS {
        if applied.contains(&version) {
            continue;
        }
        let mut tx = pool.begin().await?;
        for stmt in statements(sql) {
            sqlx::query(stmt).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
            .bind(version)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

pub(crate) fn statements(sql: &str) -> impl Iterator<Item = &str> {
    sql.split(';').map(str::trim).filter(|s| !s.is_empty())
}
