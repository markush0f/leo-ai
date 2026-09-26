use ira_llm::ToolSpec;
use serde_json::{Value, json};

use crate::client::Client;
use crate::error::Error;

pub fn list_tasks_spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_list_tasks".into(),
        description: "Lista tareas del tablero Veritas Kanban.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "status": { "type": "string", "enum": ["todo", "in-progress", "blocked", "done"] },
                "type": { "type": "string", "enum": ["code", "research", "content", "automation"] },
                "project": { "type": "string" },
                "sprint": { "type": "string" }
            }
        }),
    }
}

pub fn get_task_spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_get_task".into(),
        description: "Lee una tarea de Veritas Kanban por id exacto o sufijo único.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" }
            },
            "required": ["id"]
        }),
    }
}

pub fn create_task_spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_create_task".into(),
        description: "Crea una tarea en Veritas Kanban.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "type": { "type": "string", "enum": ["code", "research", "content", "automation"] },
                "priority": { "type": "string", "enum": ["low", "medium", "high"] },
                "project": { "type": "string" },
                "sprint": { "type": "string" },
                "commitPolicy": { "type": "string", "enum": ["forbidden", "allowed", "required"] }
            },
            "required": ["title"]
        }),
    }
}

pub fn update_task_spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_update_task".into(),
        description: "Actualiza campos de una tarea de Veritas Kanban.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "description": { "type": "string" },
                "status": { "type": "string", "enum": ["todo", "in-progress", "blocked", "done"] },
                "type": { "type": "string", "enum": ["code", "research", "content", "automation"] },
                "priority": { "type": "string", "enum": ["low", "medium", "high"] },
                "project": { "type": "string" },
                "sprint": { "type": "string" },
                "commitPolicy": { "type": "string", "enum": ["forbidden", "allowed", "required"] }
            },
            "required": ["id"]
        }),
    }
}

pub fn archive_task_spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_archive_task".into(),
        description: "Archiva una tarea completada en Veritas Kanban.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" }
            },
            "required": ["id"]
        }),
    }
}

pub fn summary_spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_summary".into(),
        description: "Resumen del tablero Veritas Kanban: conteos, proyectos y prioridad alta."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn call(client: &Client, tool: &str, args: Value) -> Result<Value, Error> {
    crate::invoke::run(client, tool, args).await
}

pub async fn list_tasks(client: &Client, args: Value) -> Result<Value, Error> {
    call(
        client,
        "list_tasks",
        only(&args, &["status", "type", "project", "sprint"]),
    )
    .await
}

pub async fn get_task(client: &Client, args: Value) -> Result<Value, Error> {
    require_id(&args)?;
    call(client, "get_task", only(&args, &["id"])).await
}

pub async fn create_task(client: &Client, args: Value) -> Result<Value, Error> {
    require_title(&args)?;
    call(
        client,
        "create_task",
        only(
            &args,
            &[
                "title",
                "type",
                "priority",
                "project",
                "sprint",
                "commitPolicy",
            ],
        ),
    )
    .await
}

pub async fn update_task(client: &Client, args: Value) -> Result<Value, Error> {
    require_id(&args)?;
    call(
        client,
        "update_task",
        only(
            &args,
            &[
                "id",
                "title",
                "description",
                "status",
                "type",
                "priority",
                "project",
                "sprint",
                "commitPolicy",
            ],
        ),
    )
    .await
}

pub async fn archive_task(client: &Client, args: Value) -> Result<Value, Error> {
    require_id(&args)?;
    call(client, "archive_task", only(&args, &["id"])).await
}

pub async fn summary(client: &Client) -> Result<Value, Error> {
    call(client, "get_summary", json!({})).await
}

pub(crate) fn require_id(args: &Value) -> Result<(), Error> {
    let id = args.get("id").and_then(Value::as_str).unwrap_or("").trim();
    if id.is_empty() {
        return Err(Error::msg("falta id"));
    }
    Ok(())
}

pub(crate) fn require_title(args: &Value) -> Result<(), Error> {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if title.is_empty() {
        return Err(Error::msg("falta title"));
    }
    Ok(())
}

pub(crate) fn only(args: &Value, keys: &[&str]) -> Value {
    let Some(obj) = args.as_object() else {
        return json!({});
    };
    let mut out = serde_json::Map::new();
    for key in keys {
        if let Some(value) = obj.get(*key)
            && !value.is_null()
        {
            out.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(out)
}
