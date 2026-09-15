//! Telegram command routing and in-memory conversation state.
//!
//! Produces outcomes for the runner to execute rather than performing Telegram
//! HTTP requests. Plain text starts model turns only in private chats; group
//! chats are command-only. The runner applies user authorization separately.

use leo_llm::ChatMessage;
use leo_store::{DbOp, Snapshot};

pub const MAX_TG_TEXT: usize = 4000;
pub const MAX_HISTORY: usize = 40;

pub const HELP: &str = "\
Leo — escribe y te responde el modelo activo (el mismo catálogo que `leo`).

/status — proveedor y modelo
/model [nombre] — listar o cambiar modelo
/providers [nombre] — listar o cambiar proveedor
/clear — nueva conversación
/system [texto] — ver o cambiar el prompt";

#[derive(Debug, Default)]
pub struct Session {
    pub history: Vec<ChatMessage>,
}

#[derive(Debug)]
/// Routing decision for the transport runner, including deferred database operations.
pub enum Outcome {
    Ignore,
    Text(String),
    AskLlm,
    Db { op: DbOp, note: String },
}

pub fn parse_command(text: &str) -> Option<(&str, &str)> {
    let t = text.trim();
    let rest = t.strip_prefix('/')?;
    let (cmd, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let cmd = cmd.split('@').next().unwrap_or(cmd);
    if cmd.is_empty() {
        return None;
    }
    Some((cmd, args.trim()))
}

/// Routes text and updates history; callers must authorize the sender first.
pub fn on_text(session: &mut Session, snap: &Snapshot, text: &str, private: bool) -> Outcome {
    let text = text.trim();
    if text.is_empty() {
        return Outcome::Ignore;
    }
    if let Some((cmd, args)) = parse_command(text) {
        return on_command(session, snap, cmd, args);
    }
    if !private {
        return Outcome::Ignore;
    }
    session.history.push(ChatMessage::user(text));
    cap_history(&mut session.history);
    Outcome::AskLlm
}

fn on_command(session: &mut Session, snap: &Snapshot, cmd: &str, args: &str) -> Outcome {
    match cmd {
        "start" | "help" => Outcome::Text(HELP.into()),
        "status" => Outcome::Text(status_line(snap)),
        "clear" => {
            session.history.clear();
            Outcome::Text("conversación nueva".into())
        }
        "system" => {
            if args.is_empty() {
                let prompt = snap.system.trim();
                if prompt.is_empty() {
                    Outcome::Text("sin system prompt. /system <texto> para ponerlo".into())
                } else {
                    Outcome::Text(prompt.into())
                }
            } else {
                Outcome::Db {
                    op: DbOp::SetSystem(args.into()),
                    note: "system prompt actualizado".into(),
                }
            }
        }
        "model" => command_model(snap, args),
        "providers" | "provider" => command_provider(snap, args),
        _ => Outcome::Text(format!("comando desconocido: /{cmd}\n{HELP}")),
    }
}

fn command_model(snap: &Snapshot, args: &str) -> Outcome {
    if args.is_empty() {
        return Outcome::Text(list_models(snap, None));
    }
    let hits = match_models(snap, args);
    match hits.len() {
        0 => Outcome::Text(format!(
            "no hay modelo «{args}»\n{}",
            list_models(snap, None)
        )),
        1 => {
            let m = hits[0];
            let provider = provider_name(snap, m.provider_id);
            Outcome::Db {
                op: DbOp::ActivateModel(m.id),
                note: format!("modelo {provider}/{}", m.name),
            }
        }
        _ => Outcome::Text(list_models(snap, Some(args))),
    }
}

fn command_provider(snap: &Snapshot, args: &str) -> Outcome {
    if args.is_empty() {
        return Outcome::Text(list_providers(snap, None));
    }
    let hits = match_providers(snap, args);
    match hits.len() {
        0 => Outcome::Text(format!(
            "no hay proveedor «{args}»\n{}",
            list_providers(snap, None)
        )),
        1 => {
            let p = hits[0];
            Outcome::Db {
                op: DbOp::ActivateProvider(p.id),
                note: format!("proveedor {}", p.name),
            }
        }
        _ => Outcome::Text(list_providers(snap, Some(args))),
    }
}

pub fn status_line(snap: &Snapshot) -> String {
    match (snap.active_provider(), snap.active_model()) {
        (Some(p), Some(m)) => format!("{}/{}", p.name, m.name),
        _ => "sin modelo activo — /model".into(),
    }
}

fn list_models(snap: &Snapshot, query: Option<&str>) -> String {
    let rows: Vec<_> = snap
        .models
        .iter()
        .filter(|m| query.map_or(true, |q| model_matches(snap, m, q)))
        .collect();
    if rows.is_empty() {
        return "no hay modelos en el catálogo".into();
    }
    let mut out = String::from("modelos:");
    for m in rows {
        let mark = if Some(m.id) == snap.active_model_id {
            "*"
        } else {
            " "
        };
        let provider = provider_name(snap, m.provider_id);
        out.push_str(&format!("\n{mark} {}  ({provider})", m.name));
    }
    out
}

fn list_providers(snap: &Snapshot, query: Option<&str>) -> String {
    let rows: Vec<_> = snap
        .providers
        .iter()
        .filter(|p| query.map_or(true, |q| provider_matches(p, q)))
        .collect();
    if rows.is_empty() {
        return "no hay proveedores en el catálogo".into();
    }
    let active = snap.active_provider().map(|p| p.id);
    let mut out = String::from("proveedores:");
    for p in rows {
        let mark = if Some(p.id) == active { "*" } else { " " };
        let n = snap.models_of(p.id).len();
        out.push_str(&format!("\n{mark} {}  ({n} modelos)", p.name));
    }
    out
}

fn match_models<'a>(snap: &'a Snapshot, q: &str) -> Vec<&'a leo_store::ModelRow> {
    let exact: Vec<_> = snap
        .models
        .iter()
        .filter(|m| m.name.eq_ignore_ascii_case(q))
        .collect();
    if exact.len() == 1 {
        return exact;
    }
    let hits: Vec<_> = snap
        .models
        .iter()
        .filter(|m| model_matches(snap, m, q))
        .collect();
    hits
}

fn match_providers<'a>(snap: &'a Snapshot, q: &str) -> Vec<&'a leo_store::ProviderRow> {
    let exact: Vec<_> = snap
        .providers
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(q))
        .collect();
    if exact.len() == 1 {
        return exact;
    }
    snap.providers
        .iter()
        .filter(|p| provider_matches(p, q))
        .collect()
}

fn model_matches(snap: &Snapshot, model: &leo_store::ModelRow, q: &str) -> bool {
    let q = q.to_ascii_lowercase();
    let name = model.name.to_ascii_lowercase();
    let provider = provider_name(snap, model.provider_id).to_ascii_lowercase();
    name == q
        || name.starts_with(&q)
        || name.contains(&q)
        || provider == q
        || format!("{provider}/{name}").contains(&q)
}

fn provider_matches(provider: &leo_store::ProviderRow, q: &str) -> bool {
    let q = q.to_ascii_lowercase();
    let name = provider.name.to_ascii_lowercase();
    name == q || name.starts_with(&q) || name.contains(&q)
}

fn provider_name(snap: &Snapshot, id: uuid::Uuid) -> String {
    snap.providers
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "—".into())
}

pub fn cap_history(history: &mut Vec<ChatMessage>) {
    if history.len() > MAX_HISTORY {
        let drop = history.len() - MAX_HISTORY;
        history.drain(0..drop);
    }
}

pub fn split_telegram(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec!["(vacío)".into()];
    }
    if text.chars().count() <= MAX_TG_TEXT {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut buf = String::new();
    for line in text.split_inclusive('\n') {
        if buf.chars().count() + line.chars().count() > MAX_TG_TEXT && !buf.is_empty() {
            chunks.push(std::mem::take(&mut buf));
        }
        if line.chars().count() > MAX_TG_TEXT {
            if !buf.is_empty() {
                chunks.push(std::mem::take(&mut buf));
            }
            let mut rest: String = line.to_string();
            while rest.chars().count() > MAX_TG_TEXT {
                let take: String = rest.chars().take(MAX_TG_TEXT).collect();
                rest = rest.chars().skip(MAX_TG_TEXT).collect();
                chunks.push(take);
            }
            buf = rest;
        } else {
            buf.push_str(line);
        }
    }
    if !buf.is_empty() {
        chunks.push(buf);
    }
    chunks
}

pub fn parse_allow_users(raw: &str) -> Vec<i64> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() { None } else { s.parse().ok() }
        })
        .collect()
}

pub fn allowed(allow: &[i64], user_id: i64) -> bool {
    !allow.is_empty() && allow.contains(&user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leo_store::stub_snapshot;
    use uuid::Uuid;

    fn snap() -> Snapshot {
        let mut s = stub_snapshot("grok", "grok-4.6", "sé breve");
        let pid = s.providers[0].id;
        s.models.push(leo_store::ModelRow {
            id: Uuid::from_u128(3),
            provider_id: pid,
            name: "grok-4.5".into(),
        });
        let other = Uuid::from_u128(9);
        s.providers.push(leo_store::ProviderRow {
            id: other,
            name: "ollama".into(),
            kind: "ollama".into(),
            base_url: None,
            api_key: None,
        });
        s.models.push(leo_store::ModelRow {
            id: Uuid::from_u128(10),
            provider_id: other,
            name: "gemma3:latest".into(),
        });
        s
    }

    #[test]
    fn parse_strips_bot_username() {
        assert_eq!(parse_command("/model@LeoBot grok"), Some(("model", "grok")));
        assert_eq!(parse_command("  /status  "), Some(("status", "")));
        assert_eq!(parse_command("hola"), None);
    }

    #[test]
    fn private_text_queues_llm() {
        let mut session = Session::default();
        let snap = snap();
        assert!(matches!(
            on_text(&mut session, &snap, "hola", true),
            Outcome::AskLlm
        ));
        assert_eq!(session.history.len(), 1);
    }

    #[test]
    fn group_plain_text_is_ignored() {
        let mut session = Session::default();
        let snap = snap();
        assert!(matches!(
            on_text(&mut session, &snap, "hola", false),
            Outcome::Ignore
        ));
        assert!(session.history.is_empty());
    }

    #[test]
    fn model_switch_is_unique() {
        let mut session = Session::default();
        let snap = snap();
        match on_text(&mut session, &snap, "/model grok-4.5", true) {
            Outcome::Db {
                op: DbOp::ActivateModel(id),
                note,
            } => {
                assert_eq!(id, Uuid::from_u128(3));
                assert!(note.contains("grok-4.5"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn model_prefix_lists_when_ambiguous() {
        let mut session = Session::default();
        let snap = snap();
        match on_text(&mut session, &snap, "/model grok", true) {
            Outcome::Text(t) => {
                assert!(t.contains("grok-4.6"));
                assert!(t.contains("grok-4.5"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn provider_switch() {
        let mut session = Session::default();
        let snap = snap();
        match on_text(&mut session, &snap, "/providers ollama", true) {
            Outcome::Db {
                op: DbOp::ActivateProvider(id),
                ..
            } => assert_eq!(id, Uuid::from_u128(9)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn clear_resets_history() {
        let mut session = Session::default();
        let snap = snap();
        on_text(&mut session, &snap, "hola", true);
        on_text(&mut session, &snap, "/clear", true);
        assert!(session.history.is_empty());
    }

    #[test]
    fn allowlist_denies_empty_and_unknown() {
        assert!(!allowed(&[], 1));
        assert!(!allowed(&[2, 3], 1));
        assert!(allowed(&[1, 2], 1));
        assert_eq!(parse_allow_users("1, 2\n3"), vec![1, 2, 3]);
    }

    #[test]
    fn split_long_reply() {
        let text = "a".repeat(MAX_TG_TEXT + 10);
        let chunks = split_telegram(&text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), MAX_TG_TEXT);
        assert_eq!(chunks[1], "a".repeat(10));
    }
}
