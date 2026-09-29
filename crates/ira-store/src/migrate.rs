use sqlx::PgPool;

const SCHEMA: &str = include_str!("../../../deploy/postgres/init.sql");
const MIGRATION_001: &str =
    include_str!("../../../deploy/postgres/migrations/001_engines_conversations.sql");
const MIGRATION_002: &str =
    include_str!("../../../deploy/postgres/migrations/002_codex_provider.sql");
const MIGRATION_003: &str =
    include_str!("../../../deploy/postgres/migrations/003_database_connections.sql");
const MIGRATION_004: &str = include_str!("../../../deploy/postgres/migrations/004_rename_ira.sql");
const MIGRATION_005: &str =
    include_str!("../../../deploy/postgres/migrations/005_whatsapp_channel.sql");
const MIGRATION_006: &str =
    include_str!("../../../deploy/postgres/migrations/006_host_services.sql");
const MIGRATION_007: &str =
    include_str!("../../../deploy/postgres/migrations/007_model_effort.sql");
const MIGRATION_008: &str = include_str!("../../../deploy/postgres/migrations/008_mcp_manager.sql");
const MIGRATION_009: &str = include_str!("../../../deploy/postgres/migrations/009_mcp_config.sql");
const MIGRATION_010: &str =
    include_str!("../../../deploy/postgres/migrations/010_drop_whatsapp.sql");
const MIGRATION_011: &str =
    include_str!("../../../deploy/postgres/migrations/011_secrets_and_tool_policy.sql");
const MIGRATION_012: &str = include_str!("../../../deploy/postgres/migrations/012_drop_veritas.sql");

const MIGRATIONS: &[(i32, &str)] = &[
    (1, MIGRATION_001),
    (2, MIGRATION_002),
    (3, MIGRATION_003),
    (4, MIGRATION_004),
    (5, MIGRATION_005),
    (6, MIGRATION_006),
    (7, MIGRATION_007),
    (8, MIGRATION_008),
    (9, MIGRATION_009),
    (10, MIGRATION_010),
    (11, MIGRATION_011),
    (12, MIGRATION_012),
];

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
        return seal_keys(pool).await;
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
    seal_keys(pool).await
}

async fn seal_keys(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    crate::databases::seal_plaintext_api_keys(pool)
        .await
        .map_err(|err| match err {
            crate::databases::DatabaseError::Sql(err) => err,
            other => sqlx::Error::Protocol(other.to_string()),
        })
}

pub(crate) fn statements(sql: &str) -> impl Iterator<Item = &str> {
    sql.split(';').map(str::trim).filter(|s| !s.is_empty())
}
