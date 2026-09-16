use leo_llm::{ChatMessage, ToolCall};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::is_unique_violation;

pub const CONTEXT_LIMIT: i64 = 80;
pub const CHANNEL_LOCAL: &str = "local";
pub const CHANNEL_TELEGRAM: &str = "telegram";
pub const CHANNEL_VOICE: &str = "voice";

#[derive(Debug, Clone)]
pub struct ConversationRow {
    pub id: Uuid,
    pub channel: String,
    pub external_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub tool_calls: serde_json::Value,
    pub model_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub tool_calls: serde_json::Value,
    pub model_id: Option<Uuid>,
}

impl NewMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: serde_json::json!([]),
            model_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>, model_id: Option<Uuid>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: serde_json::json!([]),
            model_id,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            role: "error".into(),
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: serde_json::json!([]),
            model_id: None,
        }
    }
}

pub async fn list_conversations(
    pool: &PgPool,
    channel: &str,
) -> Result<Vec<ConversationRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, channel, external_id, title FROM conversations
         WHERE channel = $1 AND archived_at IS NULL
         ORDER BY updated_at DESC",
    )
    .bind(channel)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(conversation_from_row)
    .collect()
}

pub async fn get_conversation(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<ConversationRow>, sqlx::Error> {
    sqlx::query("SELECT id, channel, external_id, title FROM conversations WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(conversation_from_row)
        .transpose()
}

pub async fn conversation_messages(
    pool: &PgPool,
    conversation_id: Uuid,
) -> Result<Vec<MessageRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, conversation_id, role, content, tool_call_id, name, tool_calls, model_id
         FROM messages WHERE conversation_id = $1
         ORDER BY created_at ASC, id ASC",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(message_from_row)
    .collect()
}

pub async fn context_messages(
    pool: &PgPool,
    conversation_id: Uuid,
    limit: i64,
) -> Result<Vec<ChatMessage>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, conversation_id, role, content, tool_call_id, name, tool_calls, model_id
         FROM (
            SELECT id, conversation_id, role, content, tool_call_id, name, tool_calls, model_id, created_at
            FROM messages
            WHERE conversation_id = $1 AND role IN ('user', 'assistant', 'tool')
            ORDER BY created_at DESC, id DESC
            LIMIT $2
         ) t
         ORDER BY created_at ASC, id ASC",
    )
    .bind(conversation_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let msg = message_from_row(row)?;
        if let Some(chat) = message_to_chat(&msg) {
            out.push(chat);
        }
    }
    Ok(out)
}

pub async fn create_conversation(
    pool: &PgPool,
    channel: &str,
    external_id: Option<&str>,
) -> Result<ConversationRow, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO conversations (id, channel, external_id) VALUES ($1, $2, $3)
         RETURNING id, channel, external_id, title",
    )
    .bind(id)
    .bind(channel)
    .bind(external_id)
    .fetch_one(pool)
    .await
    .and_then(conversation_from_row)
}

pub async fn archive_conversation(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE conversations SET archived_at = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_active_conversation(pool: &PgPool, id: Option<Uuid>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE settings SET active_conversation_id = $1 WHERE id = 1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn ensure_local(pool: &PgPool) -> Result<ConversationRow, sqlx::Error> {
    let active: Option<Uuid> =
        sqlx::query_scalar("SELECT active_conversation_id FROM settings WHERE id = 1")
            .fetch_optional(pool)
            .await?
            .flatten();
    if let Some(id) = active {
        if let Some(row) = live_local(pool, id).await? {
            return Ok(row);
        }
    }
    let created = create_conversation(pool, CHANNEL_LOCAL, None).await?;
    set_active_conversation(pool, Some(created.id)).await?;
    Ok(created)
}

pub async fn new_local(pool: &PgPool) -> Result<ConversationRow, sqlx::Error> {
    let created = create_conversation(pool, CHANNEL_LOCAL, None).await?;
    set_active_conversation(pool, Some(created.id)).await?;
    Ok(created)
}

pub async fn ensure_telegram(pool: &PgPool, chat_id: i64) -> Result<ConversationRow, sqlx::Error> {
    let external = chat_id.to_string();
    if let Some(row) = live_telegram(pool, &external).await? {
        return Ok(row);
    }
    match create_conversation(pool, CHANNEL_TELEGRAM, Some(&external)).await {
        Ok(row) => Ok(row),
        Err(err) if is_unique_violation(&err) => live_telegram(pool, &external).await?.ok_or(err),
        Err(err) => Err(err),
    }
}

pub async fn new_telegram(pool: &PgPool, chat_id: i64) -> Result<ConversationRow, sqlx::Error> {
    let external = chat_id.to_string();
    if let Some(row) = live_telegram(pool, &external).await? {
        archive_conversation(pool, row.id).await?;
    }
    create_conversation(pool, CHANNEL_TELEGRAM, Some(&external)).await
}

pub async fn ensure_voice(pool: &PgPool) -> Result<ConversationRow, sqlx::Error> {
    if let Some(row) = live_voice(pool).await? {
        return Ok(row);
    }
    match create_conversation(pool, CHANNEL_VOICE, None).await {
        Ok(row) => Ok(row),
        Err(err) if is_unique_violation(&err) => live_voice(pool).await?.ok_or(err),
        Err(err) => Err(err),
    }
}

pub async fn append_message(
    pool: &PgPool,
    conversation_id: Uuid,
    msg: NewMessage,
) -> Result<MessageRow, sqlx::Error> {
    let id = Uuid::new_v4();
    let row = sqlx::query(
        "INSERT INTO messages (
            id, conversation_id, role, content, tool_call_id, name, tool_calls, model_id
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, conversation_id, role, content, tool_call_id, name, tool_calls, model_id",
    )
    .bind(id)
    .bind(conversation_id)
    .bind(&msg.role)
    .bind(&msg.content)
    .bind(&msg.tool_call_id)
    .bind(&msg.name)
    .bind(&msg.tool_calls)
    .bind(msg.model_id)
    .fetch_one(pool)
    .await?;

    sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1")
        .bind(conversation_id)
        .execute(pool)
        .await?;

    if msg.role == "user" {
        maybe_title(pool, conversation_id, &msg.content).await?;
    }
    message_from_row(row)
}

pub fn display_kind(role: &str) -> Option<&'static str> {
    match role {
        "user" => Some("user"),
        "assistant" => Some("leo"),
        "error" => Some("error"),
        _ => None,
    }
}

async fn maybe_title(
    pool: &PgPool,
    conversation_id: Uuid,
    content: &str,
) -> Result<(), sqlx::Error> {
    let title = title_from(content);
    if title.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE conversations SET title = $2
         WHERE id = $1 AND title IS NULL",
    )
    .bind(conversation_id)
    .bind(title)
    .execute(pool)
    .await?;
    Ok(())
}

fn title_from(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or("").trim();
    let mut chars = line.chars();
    let taken: String = chars.by_ref().take(60).collect();
    if chars.next().is_some() {
        format!("{taken}…")
    } else {
        taken
    }
}

async fn live_local(pool: &PgPool, id: Uuid) -> Result<Option<ConversationRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, channel, external_id, title FROM conversations
         WHERE id = $1 AND channel = 'local' AND archived_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .map(conversation_from_row)
    .transpose()
}

async fn live_telegram(
    pool: &PgPool,
    external_id: &str,
) -> Result<Option<ConversationRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, channel, external_id, title FROM conversations
         WHERE channel = 'telegram' AND external_id = $1 AND archived_at IS NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(external_id)
    .fetch_optional(pool)
    .await?
    .map(conversation_from_row)
    .transpose()
}

async fn live_voice(pool: &PgPool) -> Result<Option<ConversationRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, channel, external_id, title FROM conversations
         WHERE channel = 'voice' AND archived_at IS NULL
         ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .map(conversation_from_row)
    .transpose()
}

fn conversation_from_row(row: sqlx::postgres::PgRow) -> Result<ConversationRow, sqlx::Error> {
    Ok(ConversationRow {
        id: row.get("id"),
        channel: row.get("channel"),
        external_id: row.get("external_id"),
        title: row.get("title"),
    })
}

fn message_from_row(row: sqlx::postgres::PgRow) -> Result<MessageRow, sqlx::Error> {
    Ok(MessageRow {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        role: row.get("role"),
        content: row.get("content"),
        tool_call_id: row.get("tool_call_id"),
        name: row.get("name"),
        tool_calls: row.get("tool_calls"),
        model_id: row.get("model_id"),
    })
}

fn message_to_chat(msg: &MessageRow) -> Option<ChatMessage> {
    match msg.role.as_str() {
        "user" => Some(ChatMessage::user(msg.content.clone())),
        "assistant" => {
            let calls = parse_tool_calls(&msg.tool_calls);
            if calls.is_empty() {
                Some(ChatMessage::assistant(msg.content.clone()))
            } else {
                Some(ChatMessage::assistant_tools(msg.content.clone(), calls))
            }
        }
        "tool" => Some(ChatMessage::tool(
            msg.tool_call_id.clone().unwrap_or_default(),
            msg.name.clone().unwrap_or_default(),
            msg.content.clone(),
        )),
        _ => None,
    }
}

fn parse_tool_calls(value: &serde_json::Value) -> Vec<ToolCall> {
    serde_json::from_value(value.clone()).unwrap_or_default()
}
