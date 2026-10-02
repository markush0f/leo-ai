use ira_llm::ToolSpec;
use ira_store::MemoryError;
use sqlx::PgPool;

use crate::error::ToolError;
use crate::registry::Registry;

/// Adds remember, recall, and forget tools bound to the Ira database.
pub fn attach_memory(registry: Registry, pool: PgPool) -> Registry {
    if registry.has("remember_fact") {
        return registry;
    }
    let mut builder = registry.builder_from();
    let save = pool.clone();
    builder.add_fn(remember_spec(), move |_ctx, args| {
        let pool = save.clone();
        async move {
            let content = arg(&args, "content")?;
            ira_store::remember(&pool, &content)
                .await
                .map(|memory| format!("Guardado (id {}): {}", memory.id, memory.content))
                .map_err(tool_err)
        }
    });
    let find = pool.clone();
    builder.add_fn(recall_spec(), move |_ctx, args| {
        let pool = find.clone();
        async move {
            let query = args
                .get("query")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let found = ira_store::recall_memory(&pool, &query, 8)
                .await
                .map_err(tool_err)?;
            if found.is_empty() {
                return Ok("No hay memorias que coincidan.".into());
            }
            Ok(found
                .iter()
                .map(|memory| format!("[{}] {}", memory.id, memory.content))
                .collect::<Vec<_>>()
                .join("\n"))
        }
    });
    builder.add_fn(forget_spec(), move |_ctx, args| {
        let pool = pool.clone();
        async move {
            let query = arg(&args, "query")?;
            let removed = ira_store::forget_memory(&pool, &query)
                .await
                .map_err(tool_err)?;
            if removed.is_empty() {
                return Ok("No había una memoria que coincidiera.".into());
            }
            Ok(format!(
                "Olvidado: {}",
                removed
                    .iter()
                    .map(|memory| memory.content.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ))
        }
    });
    builder.build()
}

fn remember_spec() -> ToolSpec {
    ToolSpec {
        name: "remember_fact".into(),
        description: "Guarda un dato cuando el usuario pide recordarlo, anotarlo, memorizarlo o quedarse con ello, con cualquier formulación. Pasa solo el hecho, no la orden."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "Hecho a guardar" }
            },
            "required": ["content"]
        }),
    }
}

fn recall_spec() -> ToolSpec {
    ToolSpec {
        name: "recall_memory".into(),
        description: "Busca datos que el usuario pidió guardar. Query vacío lista los más recientes."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Texto a buscar" }
            }
        }),
    }
}

fn forget_spec() -> ToolSpec {
    ToolSpec {
        name: "forget_memory".into(),
        description: "Borra un dato guardado cuando el usuario pide olvidarlo, borrarlo o que dejes de recordarlo, con cualquier formulación. Pasa el hecho o su id."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Hecho o id a olvidar" }
            },
            "required": ["query"]
        }),
    }
}

fn arg(args: &serde_json::Value, name: &'static str) -> Result<String, ToolError> {
    args.get(name)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(ToolError::MissingArg(name))
}

fn tool_err(err: MemoryError) -> ToolError {
    ToolError::from_display(err)
}
