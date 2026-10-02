use sqlx::PgPool;
use uuid::Uuid;

const MAX_CONTENT: usize = 8000;

#[derive(Debug, Clone)]
pub struct Memory {
    pub id: Uuid,
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("el dato está vacío")]
    Empty,
    #[error("el dato supera {MAX_CONTENT} caracteres")]
    TooLong,
}

pub const INSTRUCTION: &str = "Decide tú si el usuario pide guardar o borrar un dato. No dependas de una frase exacta: valen «recuerda que», «anota», «quédate con esto», «no te olvides», «guarda», «memoriza», «olvida», «borra eso» y cualquier formulación parecida. Si pide recordarlo, llama remember_fact con el hecho, no con la orden. Si pide olvidarlo, llama forget_memory. Si solo pregunta, usa la memoria de abajo o recall_memory; no guardes nada que no haya pedido. No inventes datos guardados ni guardes la contraseña de Ira ni claves de API.";

pub async fn remember(pool: &PgPool, content: &str) -> Result<Memory, MemoryError> {
    let content = normalize(content)?;
    if let Some(existing) = find_exact(pool, &content).await? {
        sqlx::query("UPDATE memories SET updated_at = now() WHERE id = $1")
            .bind(existing.id)
            .execute(pool)
            .await?;
        return Ok(existing);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO memories (id, content) VALUES ($1, $2)")
        .bind(id)
        .bind(&content)
        .execute(pool)
        .await?;
    Ok(Memory { id, content })
}

pub async fn recall_memory(pool: &PgPool, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError> {
    let limit = limit.clamp(1, 20);
    let query = query.trim();
    if query.is_empty() {
        let rows = sqlx::query("SELECT id, content FROM memories ORDER BY updated_at DESC LIMIT $1")
            .bind(limit)
            .fetch_all(pool)
            .await?;
        return Ok(rows.into_iter().map(memory_from_row).collect());
    }
    Ok(search(pool, query, limit)
        .await?
        .into_iter()
        .map(|(memory, _)| memory)
        .collect())
}

pub async fn forget_memory(pool: &PgPool, query: &str) -> Result<Vec<Memory>, MemoryError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(MemoryError::Empty);
    }
    if let Ok(id) = Uuid::parse_str(query) {
        let row = sqlx::query("DELETE FROM memories WHERE id = $1 RETURNING id, content")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        return Ok(row.into_iter().map(memory_from_row).collect());
    }
    let hits = search(pool, query, 20).await?;
    let Some(best) = hits.iter().map(|hit| hit.1).max() else {
        return Ok(Vec::new());
    };
    if best == 0 {
        return Ok(Vec::new());
    }
    let mut removed = Vec::new();
    for (memory, score) in hits {
        if score != best {
            continue;
        }
        sqlx::query("DELETE FROM memories WHERE id = $1")
            .bind(memory.id)
            .execute(pool)
            .await?;
        removed.push(memory);
    }
    Ok(removed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryNote {
    pub action: &'static str,
    pub content: String,
}

pub struct TurnPrep {
    pub prompt: String,
    pub notes: Vec<MemoryNote>,
}

pub async fn prepare_turn(pool: &PgPool, user_text: &str) -> Result<TurnPrep, MemoryError> {
    let found = search(pool, user_text, 8).await?;
    let prompt = if found.is_empty() {
        String::new()
    } else {
        let lines = found
            .iter()
            .map(|(memory, _)| format!("- [{}] {}", memory.id, memory.content))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Memoria que el usuario pidió guardar. Úsala para responder. No digas que no lo recuerdas si está aquí:\n{lines}"
        )
    };
    Ok(TurnPrep {
        prompt,
        notes: Vec::new(),
    })
}

fn normalize(content: &str) -> Result<String, MemoryError> {
    let content = content.trim();
    if content.is_empty() {
        return Err(MemoryError::Empty);
    }
    if content.chars().count() > MAX_CONTENT {
        return Err(MemoryError::TooLong);
    }
    Ok(content.to_string())
}

async fn find_exact(pool: &PgPool, content: &str) -> Result<Option<Memory>, sqlx::Error> {
    sqlx::query("SELECT id, content FROM memories WHERE lower(btrim(content)) = lower($1) LIMIT 1")
        .bind(content)
        .fetch_optional(pool)
        .await
        .map(|row| row.map(memory_from_row))
}

async fn search(pool: &PgPool, query: &str, limit: i64) -> Result<Vec<(Memory, usize)>, MemoryError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let tokens = tokens(query);
    let rows = sqlx::query(
        "SELECT id, content FROM memories
         WHERE to_tsvector('simple', content) @@ plainto_tsquery('simple', $1)
            OR content ILIKE '%' || $1 || '%'
         ORDER BY updated_at DESC
         LIMIT 40",
    )
    .bind(query.trim())
    .fetch_all(pool)
    .await?;
    let mut ranked: Vec<(Memory, usize)> = rows
        .into_iter()
        .map(memory_from_row)
        .map(|memory| {
            let score = score(&memory.content, &tokens);
            (memory, score)
        })
        .filter(|(_, score)| *score > 0 || tokens.is_empty())
        .collect();
    if ranked.is_empty() && !tokens.is_empty() {
        let rows = sqlx::query("SELECT id, content FROM memories ORDER BY updated_at DESC LIMIT 200")
            .fetch_all(pool)
            .await?;
        ranked = rows
            .into_iter()
            .map(memory_from_row)
            .filter_map(|memory| {
                let score = score(&memory.content, &tokens);
                (score > 0).then_some((memory, score))
            })
            .collect();
    }
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.content.cmp(&b.0.content)));
    ranked.truncate(limit as usize);
    Ok(ranked)
}

fn memory_from_row(row: sqlx::postgres::PgRow) -> Memory {
    use sqlx::Row;
    Memory {
        id: row.get("id"),
        content: row.get("content"),
    }
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter_map(|word| {
            let word = word.trim().to_lowercase();
            if word.chars().count() < 3 || STOP.contains(&word.as_str()) {
                None
            } else {
                Some(word)
            }
        })
        .collect()
}

fn score(content: &str, tokens: &[String]) -> usize {
    if tokens.is_empty() {
        return 0;
    }
    let lower = content.to_lowercase();
    tokens.iter().filter(|token| lower.contains(token.as_str())).count()
}

const STOP: &[&str] = &[
    "que", "qué", "una", "uno", "unos", "unas", "por", "para", "con", "los", "las", "del", "como",
    "esto", "esta", "este", "ese", "esa", "eso", "mio", "mío", "mis", "tus", "the", "and", "con",
    "desde", "hasta", "sobre", "cuando", "donde", "dónde", "cual", "cuál", "cuales", "cuáles",
    "tiene", "tengo", "tienes", "puede", "puedes", "favor", "gracias", "hola", "ira",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_overlapping_tokens() {
        let tokens = tokens("cuál es mi color favorito");
        assert!(score("Tu color favorito es azul", &tokens) >= 2);
        assert_eq!(score("el tiempo en madrid", &tokens), 0);
    }

    #[tokio::test]
    async fn remembers_and_forgets_across_queries() {
        let url = crate::database_url();
        let Ok(pool) = crate::connect(&url).await else {
            return;
        };
        crate::migrate(&pool).await.expect("migrate");
        let marker = Uuid::new_v4();
        let fact = format!("el código de prueba {marker} es norte");
        let saved = remember(&pool, &fact).await.expect("save");
        let again = remember(&pool, &fact).await.expect("dedup");
        assert_eq!(saved.id, again.id);
        let found = recall_memory(&pool, &marker.to_string(), 5)
            .await
            .expect("recall");
        assert!(found.iter().any(|memory| memory.id == saved.id));
        let removed = forget_memory(&pool, &saved.id.to_string())
            .await
            .expect("forget");
        assert_eq!(removed.len(), 1);
        let found = recall_memory(&pool, &marker.to_string(), 5)
            .await
            .expect("recall after");
        assert!(found.iter().all(|memory| memory.id != saved.id));
    }
}
