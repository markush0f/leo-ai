use sqlx::{PgPool, Row};

use crate::memories::INSTRUCTION;

pub const MAX_CONTENT: usize = 8000;
pub const DEFAULT_PERSONA_CHAT: &str =
    "Eres Ira, un asistente. Responde en español, claro y directo.";
pub const DEFAULT_PERSONA_VOICE: &str =
    "Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.";
pub const DEFAULT_MEMORY: &str = INSTRUCTION;
pub const DEFAULT_TOOLS: &str = "Usa las herramientas disponibles cuando sean útiles para responder, incluidas herramientas MCP locales y externas. La lista de servicios locales no representa todos los MCP conectados: comprueba las herramientas disponibles en esta conversación y úsalas según su descripción. Para resultados deportivos actuales, consulta herramientas deportivas disponibles antes de responder. No afirmes que un MCP no está conectado si no lo verificaste intentando usar sus herramientas; si no hay herramienta pertinente o falla, explica esa limitación concreta.";
pub const DEFAULT_WEB_SEARCH: &str = "Cuando el usuario pida buscar en internet, consultar la web o información reciente, usa la herramienta web_search si está disponible. Para noticias o datos que cambian, prioriza páginas recién publicadas o actualizadas, comprueba su fecha y distingue fecha de publicación de fecha del evento. Si las fuentes no son suficientemente recientes, dilo claramente en vez de presentar datos antiguos como actuales. Haz la búsqueda en segundo plano: no abras ni controles el navegador local del usuario. Después, responde en este mismo turno con un resumen útil y menciona fuentes cuando estén disponibles; no te limites a iniciar una búsqueda ni dejes al usuario esperando.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub key: String,
    pub channel: String,
    pub content: String,
    pub active: bool,
    pub position: i32,
    pub updated_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum InstructionError {
    #[error("clave de instrucción inválida")]
    BadKey,
    #[error("canal de instrucción inválido")]
    BadChannel,
    #[error("instrucción vacía")]
    Empty,
    #[error("instrucción demasiado larga")]
    TooLong,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

pub struct PromptDynamic<'a> {
    pub model: &'a str,
    pub tool_names: &'a [String],
    pub memories: &'a str,
    pub extra: &'a str,
}

pub fn allowed(key: &str, channel: &str) -> bool {
    matches!(
        (key, channel),
        ("persona", "chat" | "voice")
            | ("memory" | "tools" | "web_search", "chat")
            | ("memory" | "tools" | "web_search", "all")
    )
}

pub fn default_content(key: &str, channel: &str) -> Option<&'static str> {
    match (key, channel) {
        ("persona", "chat") => Some(DEFAULT_PERSONA_CHAT),
        ("persona", "voice") => Some(DEFAULT_PERSONA_VOICE),
        ("memory", "chat" | "all") => Some(DEFAULT_MEMORY),
        ("tools", "chat" | "all") => Some(DEFAULT_TOOLS),
        ("web_search", "chat" | "all") => Some(DEFAULT_WEB_SEARCH),
        _ => None,
    }
}

fn default_position(key: &str) -> i32 {
    match key {
        "persona" => 10,
        "memory" => 20,
        "tools" => 30,
        "web_search" => 40,
        _ => 100,
    }
}

pub async fn seed_missing(pool: &PgPool) -> Result<(), sqlx::Error> {
    for (key, channel) in [
        ("persona", "chat"),
        ("persona", "voice"),
        ("memory", "chat"),
        ("tools", "chat"),
        ("web_search", "chat"),
    ] {
        let Some(content) = default_content(key, channel) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO instructions (key, channel, content, active, position)
             VALUES ($1, $2, $3, TRUE, $4)
             ON CONFLICT (key, channel) DO NOTHING",
        )
        .bind(key)
        .bind(channel)
        .bind(content)
        .bind(default_position(key))
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn list_instructions(pool: &PgPool) -> Result<Vec<Instruction>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT key, channel, content, active, position, updated_at::text AS updated_at
         FROM instructions
         ORDER BY position, key, channel",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(instruction_from_row).collect())
}

pub async fn upsert_instruction(
    pool: &PgPool,
    key: &str,
    channel: &str,
    content: &str,
    active: Option<bool>,
) -> Result<Instruction, InstructionError> {
    let content = validate(key, channel, content)?;
    let active = active.unwrap_or(true);
    let row = sqlx::query(
        "INSERT INTO instructions (key, channel, content, active, position)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (key, channel) DO UPDATE
         SET content = EXCLUDED.content,
             active = EXCLUDED.active,
             updated_at = now()
         RETURNING key, channel, content, active, position, updated_at::text AS updated_at",
    )
    .bind(key)
    .bind(channel)
    .bind(&content)
    .bind(active)
    .bind(default_position(key))
    .fetch_one(pool)
    .await?;
    mirror_persona(pool, key, channel, &content).await?;
    Ok(instruction_from_row(row))
}

pub async fn reset_instruction(
    pool: &PgPool,
    key: &str,
    channel: &str,
) -> Result<Instruction, InstructionError> {
    let Some(content) = default_content(key, channel) else {
        return Err(if allowed(key, channel) {
            InstructionError::BadChannel
        } else {
            InstructionError::BadKey
        });
    };
    upsert_instruction(pool, key, channel, content, Some(true)).await
}

pub async fn compose_prompt(
    pool: &PgPool,
    channel: &str,
    dynamic: &PromptDynamic<'_>,
) -> Result<String, sqlx::Error> {
    let blocks = list_instructions(pool).await?;
    Ok(compose(&blocks, channel, dynamic))
}

pub fn compose(blocks: &[Instruction], channel: &str, dynamic: &PromptDynamic<'_>) -> String {
    let mut parts = blocks
        .iter()
        .filter(|block| applies(block, channel))
        .map(|block| block.content.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    push_unique(&mut parts, &model_context(dynamic.model));
    push_unique(&mut parts, &tools_available(dynamic.tool_names));
    push_unique(&mut parts, dynamic.memories);
    push_unique(&mut parts, dynamic.extra);
    parts.join("\n\n")
}

pub fn active_content(blocks: &[Instruction], key: &str, channel: &str) -> Option<String> {
    blocks.iter().find_map(|block| {
        (block.key == key && block.channel == channel && block.active && !block.content.trim().is_empty())
            .then(|| block.content.clone())
    })
}

fn applies(block: &Instruction, channel: &str) -> bool {
    block.active
        && !block.content.trim().is_empty()
        && (block.channel == "all" || block.channel == channel)
}

fn model_context(model: &str) -> String {
    let model = if model.trim().is_empty() {
        "desconocido"
    } else {
        model.trim()
    };
    format!(
        "El modelo de IA que estás usando actualmente es \"{model}\". Si te preguntan qué modelo eres o cuál estás usando, responde de forma natural y honesta con este nombre. No digas que eres el modelo; explica que es el modelo que te impulsa."
    )
}

fn tools_available(names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    format!("Puedes usar estas herramientas: {}.", names.join(", "))
}

fn push_unique(parts: &mut Vec<String>, text: &str) {
    let text = text.trim();
    if text.is_empty() || parts.iter().any(|part| part.contains(text)) {
        return;
    }
    parts.push(text.to_string());
}

fn validate(key: &str, channel: &str, content: &str) -> Result<String, InstructionError> {
    if !allowed(key, channel) {
        return Err(if matches!(key, "persona" | "memory" | "tools" | "web_search") {
            InstructionError::BadChannel
        } else {
            InstructionError::BadKey
        });
    }
    let content = content.trim();
    if content.is_empty() {
        return Err(InstructionError::Empty);
    }
    if content.chars().count() > MAX_CONTENT {
        return Err(InstructionError::TooLong);
    }
    Ok(content.to_string())
}

async fn mirror_persona(
    pool: &PgPool,
    key: &str,
    channel: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    let column = match (key, channel) {
        ("persona", "chat") => "system_prompt",
        ("persona", "voice") => "voice_system_prompt",
        _ => return Ok(()),
    };
    let sql = format!("UPDATE settings SET {column} = $1 WHERE id = 1");
    sqlx::query(&sql).bind(content).execute(pool).await?;
    Ok(())
}

fn instruction_from_row(row: sqlx::postgres::PgRow) -> Instruction {
    Instruction {
        key: row.get("key"),
        channel: row.get("channel"),
        content: row.get("content"),
        active: row.get("active"),
        position: row.get("position"),
        updated_at: row.get("updated_at"),
    }
}

pub(crate) fn to_sqlx(err: InstructionError) -> sqlx::Error {
    match err {
        InstructionError::Sql(err) => err,
        other => sqlx::Error::Protocol(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks() -> Vec<Instruction> {
        [
            ("persona", "chat", DEFAULT_PERSONA_CHAT, 10),
            ("persona", "voice", DEFAULT_PERSONA_VOICE, 10),
            ("memory", "chat", DEFAULT_MEMORY, 20),
            ("tools", "chat", DEFAULT_TOOLS, 30),
            ("web_search", "chat", DEFAULT_WEB_SEARCH, 40),
        ]
        .into_iter()
        .map(|(key, channel, content, position)| Instruction {
            key: key.into(),
            channel: channel.into(),
            content: content.into(),
            active: true,
            position,
            updated_at: String::new(),
        })
        .collect()
    }

    #[test]
    fn chat_includes_each_rule_once_and_voice_skips_written_rules() {
        let dynamic = PromptDynamic {
            model: "grok-4.6",
            tool_names: &["remember_fact".into(), "web_search".into()],
            memories: "Memoria que el usuario pidió guardar. Úsala para responder. No digas que no lo recuerdas si está aquí:\n- [id] gato",
            extra: "Servicios locales.",
        };
        let chat = compose(&blocks(), "chat", &dynamic);
        assert_eq!(chat.matches("Decide tú si el usuario pide guardar").count(), 1);
        assert_eq!(chat.matches("Puedes usar estas herramientas:").count(), 1);
        assert!(chat.contains("claro y directo"));
        assert!(chat.contains("grok-4.6"));
        assert!(chat.contains("gato"));
        assert!(chat.contains("Servicios locales."));
        assert!(!chat.contains("asistente de voz"));

        let voice = compose(
            &blocks(),
            "voice",
            &PromptDynamic {
                model: "grok-4.6",
                tool_names: &[],
                memories: dynamic.memories,
                extra: "",
            },
        );
        assert!(voice.contains("asistente de voz"));
        assert!(voice.contains("gato"));
        assert!(voice.contains("grok-4.6"));
        assert!(!voice.contains("web_search"));
        assert!(!voice.contains("remember_fact"));
        assert!(!voice.contains("claro y directo"));
        assert!(!voice.contains("Decide tú si el usuario pide guardar"));
    }

    #[test]
    fn inactive_blocks_are_omitted() {
        let mut rows = blocks();
        rows[2].active = false;
        let prompt = compose(
            &rows,
            "chat",
            &PromptDynamic {
                model: "gpt-5.4",
                tool_names: &[],
                memories: "",
                extra: "",
            },
        );
        assert!(!prompt.contains("Decide tú si el usuario pide guardar"));
        assert!(prompt.contains("gpt-5.4"));
    }

    #[test]
    fn rejects_unknown_key_channel_and_length() {
        assert!(matches!(
            validate("grok", "chat", "hola"),
            Err(InstructionError::BadKey)
        ));
        assert!(matches!(
            validate("persona", "all", "hola"),
            Err(InstructionError::BadChannel)
        ));
        assert!(matches!(
            validate("persona", "chat", "  "),
            Err(InstructionError::Empty)
        ));
        let long = "a".repeat(MAX_CONTENT + 1);
        assert!(matches!(
            validate("persona", "chat", &long),
            Err(InstructionError::TooLong)
        ));
    }
}
