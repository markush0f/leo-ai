use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct HostServiceRow {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub position: i32,
}

pub async fn list_host_services(pool: &PgPool) -> Result<Vec<HostServiceRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, required, position FROM host_services ORDER BY position, name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| HostServiceRow {
            id: row.get("id"),
            name: row.get("name"),
            required: row.get("required"),
            position: row.get("position"),
        })
        .collect())
}

pub async fn sync_host_services(pool: &PgPool, services: &[HostServiceRow]) -> Result<(), sqlx::Error> {
    if services.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for service in services {
        sqlx::query(
            "INSERT INTO host_services (id, name, required, position)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE
             SET name = EXCLUDED.name,
                 required = EXCLUDED.required,
                 position = EXCLUDED.position",
        )
        .bind(&service.id)
        .bind(&service.name)
        .bind(service.required)
        .bind(service.position)
        .execute(&mut *tx)
        .await?;
    }
    let ids: Vec<String> = services.iter().map(|service| service.id.clone()).collect();
    sqlx::query("DELETE FROM host_services WHERE NOT (id = ANY($1))")
        .bind(&ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
