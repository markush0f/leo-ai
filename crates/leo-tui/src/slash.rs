use uuid::Uuid;

use crate::db::{DbOp, Snapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlashItem {
    Header(String),
    Command {
        key: &'static str,
        hint: &'static str,
    },
    Provider {
        id: Uuid,
        name: String,
        active: bool,
    },
    Model {
        id: Uuid,
        name: String,
        provider: String,
        active: bool,
    },
    Config,
    System,
    Clear,
}

impl SlashItem {
    pub fn selectable(&self) -> bool {
        !matches!(self, Self::Header(_))
    }

    pub fn title(&self) -> String {
        match self {
            Self::Header(title) => title.clone(),
            Self::Command { key, .. } => (*key).into(),
            Self::Provider { name, .. } | Self::Model { name, .. } => name.clone(),
            Self::Config => "config".into(),
            Self::System => "sistema".into(),
            Self::Clear => "limpiar".into(),
        }
    }

    pub fn hint(&self) -> String {
        match self {
            Self::Header(_) => String::new(),
            Self::Command { hint, .. } => (*hint).into(),
            Self::Provider { active, .. } => {
                if *active {
                    "*".into()
                } else {
                    String::new()
                }
            }
            Self::Model {
                provider, active, ..
            } => {
                if *active {
                    format!("{provider} *")
                } else {
                    provider.clone()
                }
            }
            Self::Config | Self::System | Self::Clear => String::new(),
        }
    }

    pub fn op(&self) -> Option<DbOp> {
        match self {
            Self::Model { id, .. } => Some(DbOp::ActivateModel(*id)),
            _ => None,
        }
    }

    fn haystack(&self) -> String {
        match self {
            Self::Header(_) => String::new(),
            Self::Command { key, hint, .. } => format!("{key} {hint}"),
            Self::Provider { name, .. } => name.clone(),
            Self::Model {
                name, provider, ..
            } => format!("{name} {provider}"),
            Self::Config => "config configuración settings".into(),
            Self::System => "sistema system prompt".into(),
            Self::Clear => "limpiar clear borrar".into(),
        }
        .to_lowercase()
    }
}

pub fn active(input: &str) -> bool {
    input.starts_with('/')
}

pub fn is_providers_cmd(input: &str) -> bool {
    matches!(
        parse_view(input.strip_prefix('/').unwrap_or(input)),
        View::Providers(_)
    )
}

pub fn items(snap: &Snapshot, input: &str, pick: Option<Uuid>) -> Vec<SlashItem> {
    let Some(raw) = input.strip_prefix('/') else {
        return Vec::new();
    };
    if let Some(pid) = pick {
        let query = match parse_view(raw) {
            View::Providers(q) => q,
            View::Commands(q) | View::Models(q) => q,
        };
        return models_of(snap, pid, &query);
    }
    match parse_view(raw) {
        View::Commands(q) => commands(&q),
        View::Models(q) => {
            let rows = catalog_models(snap)
                .into_iter()
                .filter(|item| q.is_empty() || item.haystack().contains(&q))
                .collect();
            section("modelos", rows)
        }
        View::Providers(q) => {
            let rows = catalog_providers(snap)
                .into_iter()
                .filter(|item| q.is_empty() || item.haystack().contains(&q))
                .collect();
            section("proveedores", rows)
        }
    }
}

pub fn first_selectable(items: &[SlashItem]) -> Option<usize> {
    items.iter().position(SlashItem::selectable)
}

pub fn step(items: &[SlashItem], cursor: usize, dir: i16) -> usize {
    let n = items.len() as i16;
    if n == 0 || first_selectable(items).is_none() {
        return 0;
    }
    let mut i = cursor as i16;
    for _ in 0..n {
        i = (i + dir).rem_euclid(n);
        if items[i as usize].selectable() {
            return i as usize;
        }
    }
    cursor
}

fn section(title: &str, rows: Vec<SlashItem>) -> Vec<SlashItem> {
    if rows.is_empty() {
        return Vec::new();
    }
    let mut out = vec![SlashItem::Header(title.into())];
    out.extend(rows);
    out
}

fn commands(q: &str) -> Vec<SlashItem> {
    let cmds = [
        SlashItem::Command {
            key: "model",
            hint: "todos los modelos",
        },
        SlashItem::Command {
            key: "providers",
            hint: "elegir proveedor",
        },
        SlashItem::Config,
        SlashItem::System,
        SlashItem::Clear,
    ];
    cmds.into_iter()
        .filter(|item| q.is_empty() || item.haystack().contains(q))
        .collect()
}

fn catalog_models(snap: &Snapshot) -> Vec<SlashItem> {
    snap.models
        .iter()
        .map(|m| {
            let provider = snap
                .providers
                .iter()
                .find(|p| p.id == m.provider_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            SlashItem::Model {
                id: m.id,
                name: m.name.clone(),
                provider,
                active: snap.active_model_id == Some(m.id),
            }
        })
        .collect()
}

fn catalog_providers(snap: &Snapshot) -> Vec<SlashItem> {
    snap.providers
        .iter()
        .map(|p| SlashItem::Provider {
            id: p.id,
            name: p.name.clone(),
            active: snap.active_provider().is_some_and(|a| a.id == p.id),
        })
        .collect()
}

fn models_of(snap: &Snapshot, provider_id: Uuid, query: &str) -> Vec<SlashItem> {
    let name = snap
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.name.as_str())
        .unwrap_or("proveedor");
    let rows: Vec<SlashItem> = snap
        .models_of(provider_id)
        .into_iter()
        .filter(|m| query.is_empty() || m.name.to_lowercase().contains(query))
        .map(|m| SlashItem::Model {
            id: m.id,
            name: m.name.clone(),
            provider: name.into(),
            active: snap.active_model_id == Some(m.id),
        })
        .collect();
    section(&format!("modelos · {name}"), rows)
}

enum View {
    Commands(String),
    Models(String),
    Providers(String),
}

fn parse_view(raw: &str) -> View {
    let q = raw.trim().to_lowercase();
    if let Some(rest) = strip_prefix_cmd(&q, &["modelos", "modelo", "models", "model"]) {
        View::Models(rest)
    } else if let Some(rest) =
        strip_prefix_cmd(&q, &["proveedores", "proveedor", "providers", "provider"])
    {
        View::Providers(rest)
    } else {
        View::Commands(q)
    }
}

fn strip_prefix_cmd(q: &str, cmds: &[&str]) -> Option<String> {
    for cmd in cmds {
        if q == *cmd {
            return Some(String::new());
        }
        let prefix = format!("{cmd} ");
        if let Some(rest) = q.strip_prefix(&prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::stub_snapshot;

    fn snap() -> Snapshot {
        let mut s = stub_snapshot("grok", "grok-4.6", "x");
        let pid = s.providers[0].id;
        s.models.push(crate::db::ModelRow {
            id: Uuid::from_u128(3),
            provider_id: pid,
            name: "grok-4.5".into(),
        });
        let other = Uuid::from_u128(9);
        s.providers.push(crate::db::ProviderRow {
            id: other,
            name: "gpt".into(),
            kind: "gpt".into(),
            base_url: None,
            api_key: None,
        });
        s.models.push(crate::db::ModelRow {
            id: Uuid::from_u128(10),
            provider_id: other,
            name: "gpt-4.1".into(),
        });
        s
    }

    #[test]
    fn root_shows_commands() {
        let items = items(&snap(), "/", None);
        assert!(items.iter().any(|i| matches!(i, SlashItem::Command { key: "model", .. })));
        assert!(items.iter().any(|i| matches!(i, SlashItem::Command { key: "providers", .. })));
        assert!(!items.iter().any(|i| matches!(i, SlashItem::Model { .. })));
    }

    #[test]
    fn model_lists_all_models() {
        let items = items(&snap(), "/model", None);
        assert!(matches!(&items[0], SlashItem::Header(h) if h == "modelos"));
        assert!(items.iter().any(|i| matches!(i, SlashItem::Model { name, .. } if name == "grok-4.6")));
        assert!(items.iter().any(|i| matches!(i, SlashItem::Model { name, .. } if name == "gpt-4.1")));
        assert!(!items.iter().any(|i| matches!(i, SlashItem::Provider { .. })));
    }

    #[test]
    fn providers_lists_only_providers() {
        let items = items(&snap(), "/providers", None);
        assert!(items.iter().any(|i| matches!(i, SlashItem::Header(h) if h == "proveedores")));
        assert!(items.iter().any(|i| matches!(i, SlashItem::Provider { name, .. } if name == "gpt")));
        assert!(!items.iter().any(|i| matches!(i, SlashItem::Model { .. })));
    }

    #[test]
    fn picking_provider_lists_its_models() {
        let snap = snap();
        let gpt = snap.providers.iter().find(|p| p.name == "gpt").unwrap().id;
        let items = items(&snap, "/providers", Some(gpt));
        assert!(items.iter().any(|i| matches!(i, SlashItem::Header(h) if h.contains("gpt"))));
        assert!(items.iter().any(|i| matches!(i, SlashItem::Model { name, .. } if name == "gpt-4.1")));
        assert!(!items.iter().any(|i| matches!(i, SlashItem::Model { name, .. } if name == "grok-4.6")));
    }

    #[test]
    fn model_filter() {
        let items = items(&snap(), "/model grok-4.5", None);
        assert!(items.iter().any(|i| matches!(i, SlashItem::Model { name, .. } if name == "grok-4.5")));
        assert!(!items.iter().any(|i| matches!(i, SlashItem::Model { name, .. } if name == "gpt-4.1")));
    }

    #[test]
    fn step_skips_headers() {
        let items = items(&snap(), "/model", None);
        let first = first_selectable(&items).unwrap();
        assert!(matches!(items[first], SlashItem::Model { .. }));
        let next = step(&items, first, 1);
        assert!(items[next].selectable());
    }
}
