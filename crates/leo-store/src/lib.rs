use leo_llm::{Client, LlmError, ProviderId};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const SCHEMA: &str = include_str!("../../../deploy/postgres/init.sql");

pub const DEFAULT_DATABASE_URL: &str = "postgres://leo:leo@127.0.0.1:5439/leo?sslmode=disable";

#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelRow {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub providers: Vec<ProviderRow>,
    pub models: Vec<ModelRow>,
    pub active_model_id: Option<Uuid>,
    pub system: String,
}

#[derive(Debug, Clone)]
pub enum DbOp {
    ActivateProvider(Uuid),
    ActivateModel(Uuid),
    SetKind { id: Uuid, kind: String },
    SetSystem(String),
    SetApiKey { id: Uuid, api_key: String },
    SetBaseUrl { id: Uuid, base_url: String },
    NewProvider { name: String },
    NewModel { provider_id: Uuid, name: String },
    RenameProvider { id: Uuid, name: String },
    RenameModel { id: Uuid, name: String },
    DeleteProvider(Uuid),
    DeleteModel(Uuid),
}

impl Snapshot {
    pub fn active_model(&self) -> Option<&ModelRow> {
        let id = self.active_model_id?;
        self.models.iter().find(|m| m.id == id)
    }

    pub fn active_provider(&self) -> Option<&ProviderRow> {
        let model = self.active_model()?;
        self.providers.iter().find(|p| p.id == model.provider_id)
    }

    pub fn models_of(&self, provider_id: Uuid) -> Vec<&ModelRow> {
        self.models
            .iter()
            .filter(|m| m.provider_id == provider_id)
            .collect()
    }

    pub fn activate_provider(&mut self, id: Uuid) {
        if let Some(model) = self.models.iter().find(|m| m.provider_id == id) {
            self.active_model_id = Some(model.id);
        } else {
            self.active_model_id = None;
        }
    }

    pub fn client(&self) -> Result<Client, LlmError> {
        let model = self
            .active_model()
            .ok_or_else(|| LlmError::Empty("modelo"))?;
        let provider = self
            .providers
            .iter()
            .find(|p| p.id == model.provider_id)
            .ok_or_else(|| LlmError::Empty("proveedor"))?;
        let kind = ProviderId::parse(&provider.kind)?;
        Client::connect(kind, provider.api_key.clone(), provider.base_url.clone())
            .map(|c| c.with_model(model.name.clone()))
    }
}

pub fn database_url() -> String {
    std::env::var("LEO_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().max_connections(5).connect(url).await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::Error> {
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'providers'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let seeded = tables > 0
        && sqlx::query_scalar::<_, i64>("SELECT count(*) FROM providers")
            .fetch_one(pool)
            .await
            .map(|n| n > 0)
            .unwrap_or(false);

    for stmt in statements(SCHEMA) {
        if stmt.starts_with("INSERT") && seeded {
            continue;
        }
        sqlx::query(stmt).execute(pool).await?;
    }
    Ok(())
}

pub async fn load(pool: &PgPool) -> Result<Snapshot, sqlx::Error> {
    let providers =
        sqlx::query("SELECT id, name, kind, base_url, api_key FROM providers ORDER BY name")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| ProviderRow {
                id: row.get("id"),
                name: row.get("name"),
                kind: row.get("kind"),
                base_url: row.get("base_url"),
                api_key: row.get("api_key"),
            })
            .collect();

    let models = sqlx::query("SELECT id, provider_id, name FROM models ORDER BY name")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| ModelRow {
            id: row.get("id"),
            provider_id: row.get("provider_id"),
            name: row.get("name"),
        })
        .collect();

    let settings = sqlx::query("SELECT active_model_id, system_prompt FROM settings WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    let (active_model_id, system) = match settings {
        Some(row) => (row.get("active_model_id"), row.get("system_prompt")),
        None => (
            None,
            "Eres Leo, un asistente. Responde en español, claro y directo.".into(),
        ),
    };

    Ok(Snapshot {
        providers,
        models,
        active_model_id,
        system,
    })
}

pub async fn apply(pool: &PgPool, op: DbOp) -> Result<Snapshot, sqlx::Error> {
    let should_sync = matches!(
        op,
        DbOp::ActivateProvider(_) | DbOp::SetBaseUrl { .. } | DbOp::SetKind { .. }
    );
    match op {
        DbOp::ActivateProvider(id) => {
            let model_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM models WHERE provider_id = $1 ORDER BY name LIMIT 1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?;
            set_active(pool, model_id).await?;
        }
        DbOp::ActivateModel(id) => set_active(pool, Some(id)).await?,
        DbOp::SetKind { id, kind } => {
            sqlx::query("UPDATE providers SET kind = $2 WHERE id = $1")
                .bind(id)
                .bind(kind)
                .execute(pool)
                .await?;
        }
        DbOp::SetSystem(text) => {
            sqlx::query("UPDATE settings SET system_prompt = $1 WHERE id = 1")
                .bind(text)
                .execute(pool)
                .await?;
        }
        DbOp::SetApiKey { id, api_key } => {
            let value = empty_to_none(&api_key);
            sqlx::query("UPDATE providers SET api_key = $2 WHERE id = $1")
                .bind(id)
                .bind(value)
                .execute(pool)
                .await?;
        }
        DbOp::SetBaseUrl { id, base_url } => {
            let value = empty_to_none(&base_url);
            sqlx::query("UPDATE providers SET base_url = $2 WHERE id = $1")
                .bind(id)
                .bind(value)
                .execute(pool)
                .await?;
        }
        DbOp::NewProvider { name } => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO providers (id, name, kind) VALUES ($1, $2, 'grok')")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::NewModel { provider_id, name } => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO models (id, provider_id, name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(provider_id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::RenameProvider { id, name } => {
            sqlx::query("UPDATE providers SET name = $2 WHERE id = $1")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::RenameModel { id, name } => {
            sqlx::query("UPDATE models SET name = $2 WHERE id = $1")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::DeleteProvider(id) => {
            sqlx::query("DELETE FROM providers WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
        DbOp::DeleteModel(id) => {
            sqlx::query("DELETE FROM models WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
    }
    if should_sync {
        let _ = sync_ollama_providers(pool).await;
    }
    load(pool).await
}

pub fn uses_ollama(snap: &Snapshot) -> bool {
    snap.active_provider()
        .is_some_and(|p| p.kind.eq_ignore_ascii_case("ollama"))
}

pub async fn sync_ollama_providers(pool: &PgPool) -> Result<(), String> {
    let snap = load(pool).await.map_err(|e| e.to_string())?;
    for provider in &snap.providers {
        if !provider.kind.eq_ignore_ascii_case("ollama") {
            continue;
        }
        let origin = ollama_origin(provider.base_url.as_deref());
        if provider.base_url.as_deref() != Some(origin.as_str()) {
            sqlx::query("UPDATE providers SET base_url = $2 WHERE id = $1")
                .bind(provider.id)
                .bind(&origin)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        let Ok(kind) = ProviderId::parse(&provider.kind) else {
            continue;
        };
        let Ok(client) = Client::connect(kind, provider.api_key.clone(), Some(origin)) else {
            continue;
        };
        let Ok(names) = client.list_models().await else {
            continue;
        };
        replace_models(pool, provider.id, &names)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ollama_origin(base: Option<&str>) -> String {
    let raw = base.unwrap_or("http://127.0.0.1:11434");
    let t = raw.trim().trim_end_matches('/');
    t.strip_suffix("/v1")
        .unwrap_or(t)
        .trim_end_matches('/')
        .to_string()
}

async fn replace_models(
    pool: &PgPool,
    provider_id: Uuid,
    names: &[String],
) -> Result<(), sqlx::Error> {
    let existing = sqlx::query("SELECT id, name FROM models WHERE provider_id = $1")
        .bind(provider_id)
        .fetch_all(pool)
        .await?;
    let existing: Vec<(Uuid, String)> = existing
        .into_iter()
        .map(|row| (row.get("id"), row.get("name")))
        .collect();

    let active: Option<Uuid> =
        sqlx::query_scalar::<_, Option<Uuid>>("SELECT active_model_id FROM settings WHERE id = 1")
            .fetch_optional(pool)
            .await?
            .flatten();
    let active_was_ours = existing.iter().any(|(id, _)| Some(*id) == active);
    let active_name = existing
        .iter()
        .find(|(id, _)| Some(*id) == active)
        .map(|(_, name)| name.clone());

    for name in names {
        if existing.iter().any(|(_, n)| n == name) {
            continue;
        }
        sqlx::query("INSERT INTO models (id, provider_id, name) VALUES ($1, $2, $3)")
            .bind(Uuid::new_v4())
            .bind(provider_id)
            .bind(name)
            .execute(pool)
            .await?;
    }

    for (id, name) in &existing {
        if !names.iter().any(|n| n == name) {
            sqlx::query("DELETE FROM models WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
    }

    if active_was_ours {
        let keep = active_name.filter(|n| names.iter().any(|x| x == n));
        let chosen = keep.as_deref().or_else(|| pick_preferred(names));
        if let Some(name) = chosen {
            let id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM models WHERE provider_id = $1 AND name = $2")
                    .bind(provider_id)
                    .bind(name)
                    .fetch_optional(pool)
                    .await?;
            if let Some(id) = id {
                set_active(pool, Some(id)).await?;
            }
        }
    }
    Ok(())
}

fn pick_preferred(names: &[String]) -> Option<&str> {
    names
        .iter()
        .find(|n| n.ends_with(":latest"))
        .or_else(|| names.first())
        .map(String::as_str)
}

async fn set_active(pool: &PgPool, model_id: Option<Uuid>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE settings SET active_model_id = $1 WHERE id = 1")
        .bind(model_id)
        .execute(pool)
        .await?;
    Ok(())
}

fn empty_to_none(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t) }
}

fn statements(sql: &str) -> impl Iterator<Item = &str> {
    sql.split(';').map(str::trim).filter(|s| !s.is_empty())
}

pub fn stub_snapshot(provider: &str, model: &str, system: &str) -> Snapshot {
    let pid = Uuid::from_u128(1);
    let mid = Uuid::from_u128(2);
    Snapshot {
        providers: vec![ProviderRow {
            id: pid,
            name: provider.into(),
            kind: provider.into(),
            base_url: None,
            api_key: None,
        }],
        models: vec![ModelRow {
            id: mid,
            provider_id: pid,
            name: model.into(),
        }],
        active_model_id: Some(mid),
        system: system.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_provider_picks_first_model() {
        let mut snap = stub_snapshot("grok", "grok-4.6", "x");
        let other = Uuid::from_u128(9);
        snap.providers.push(ProviderRow {
            id: other,
            name: "gpt".into(),
            kind: "gpt".into(),
            base_url: None,
            api_key: None,
        });
        snap.models.push(ModelRow {
            id: Uuid::from_u128(10),
            provider_id: other,
            name: "gpt-4.1".into(),
        });
        snap.activate_provider(other);
        assert_eq!(snap.active_model().unwrap().name, "gpt-4.1");
    }

    #[tokio::test]
    async fn postgres_migrate_roundtrip() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        let snap = load(&pool).await.expect("load");
        assert!(snap.providers.iter().any(|p| p.kind == "grok"), "seed grok");
        assert!(snap.active_model().is_some());

        let name = format!("test-{}", Uuid::new_v4());
        let snap = apply(&pool, DbOp::NewProvider { name: name.clone() })
            .await
            .expect("new provider");
        let id = snap
            .providers
            .iter()
            .find(|p| p.name == name)
            .expect("inserted")
            .id;
        apply(&pool, DbOp::DeleteProvider(id))
            .await
            .expect("cleanup");
    }

    #[test]
    fn prefers_latest_tag() {
        let names = vec!["gemma3:4b".into(), "gemma3:latest".into()];
        assert_eq!(pick_preferred(&names), Some("gemma3:latest"));
    }

    #[tokio::test]
    async fn sync_replaces_missing_ollama_model() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        let client = Client::from_env(ProviderId::Ollama).unwrap();
        let Ok(names) = client.list_models().await else {
            return;
        };
        if names.is_empty() {
            return;
        }
        sync_ollama_providers(&pool).await.expect("sync");
        let snap = load(&pool).await.expect("load");
        let ollama = snap
            .providers
            .iter()
            .find(|p| p.kind == "ollama")
            .expect("ollama provider");
        let models: Vec<String> = snap
            .models_of(ollama.id)
            .into_iter()
            .map(|m| m.name.clone())
            .collect();
        assert!(
            names.iter().all(|n| models.contains(n)),
            "sync names={names:?} db={models:?}"
        );
        assert!(!models.iter().any(|n| n == "llama3.2"));
    }
}
