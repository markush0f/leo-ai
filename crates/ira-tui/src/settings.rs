use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ira_llm::ProviderId;
use uuid::Uuid;

use crate::input::LineEdit;
use ira_store::{DbOp, Snapshot};

#[derive(Clone, Debug)]
pub enum Row {
    Header(String),
    Provider {
        id: Uuid,
        name: String,
        kind: String,
        active: bool,
    },
    Model {
        id: Uuid,
        name: String,
        active: bool,
    },
    NewProvider,
    NewModel {
        provider_id: Uuid,
    },
    Kind {
        provider_id: Uuid,
        kind: String,
    },
    ApiKey {
        provider_id: Uuid,
        status: String,
    },
    BaseUrl {
        provider_id: Uuid,
        url: String,
    },
    System {
        preview: String,
    },
    Thinking {
        on: bool,
    },
    ToolsEnabled {
        on: bool,
    },
}

impl Row {
    pub fn selectable(&self) -> bool {
        !matches!(self, Row::Header(_))
    }
}

#[derive(Clone)]
pub enum EditTarget {
    System,
    ApiKey { id: Uuid },
    BaseUrl { id: Uuid },
    NewProvider,
    NewModel { provider_id: Uuid },
    RenameProvider { id: Uuid },
    RenameModel { id: Uuid },
}

pub struct SettingsState {
    pub cursor: usize,
    pub edit: Option<(EditTarget, LineEdit)>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            cursor: 1,
            edit: None,
        }
    }

    pub fn rows(snap: &Snapshot) -> Vec<Row> {
        let mut rows = vec![Row::Header("proveedores".into())];
        for p in &snap.providers {
            let active = snap.active_provider().is_some_and(|a| a.id == p.id);
            rows.push(Row::Provider {
                id: p.id,
                name: p.name.clone(),
                kind: p.kind.clone(),
                active,
            });
        }
        rows.push(Row::NewProvider);

        if let Some(provider) = snap.active_provider() {
            rows.push(Row::Header(format!("modelos · {}", provider.name)));
            for m in snap.models_of(provider.id) {
                rows.push(Row::Model {
                    id: m.id,
                    name: m.name.clone(),
                    active: snap.active_model_id == Some(m.id),
                });
            }
            rows.push(Row::NewModel {
                provider_id: provider.id,
            });
            rows.push(Row::Header("opciones".into()));
            rows.push(Row::Kind {
                provider_id: provider.id,
                kind: provider.kind.clone(),
            });
            rows.push(Row::ApiKey {
                provider_id: provider.id,
                status: key_status(provider),
            });
            rows.push(Row::BaseUrl {
                provider_id: provider.id,
                url: provider.base_url.clone().unwrap_or_else(|| "—".into()),
            });
        } else {
            rows.push(Row::Header("modelos".into()));
            rows.push(Row::Header("sin proveedor activo".into()));
        }

        rows.push(Row::System {
            preview: preview(&snap.system, 48),
        });
        rows.push(Row::Header("chat".into()));
        rows.push(Row::Thinking {
            on: snap.settings.thinking,
        });
        rows.push(Row::ToolsEnabled {
            on: snap.settings.tools_enabled,
        });
        rows
    }

    pub fn clamp(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        if self.cursor >= rows.len() {
            self.cursor = rows.len() - 1;
        }
        if !rows[self.cursor].selectable() {
            self.move_by(rows, 1);
        }
    }

    pub fn on_key(&mut self, key: KeyEvent, snap: &Snapshot) -> Option<DbOp> {
        let rows = Self::rows(snap);
        if let Some((target, line)) = self.edit.as_mut() {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
            {
                return None;
            }
            match key.code {
                KeyCode::Esc => {
                    self.edit = None;
                    return None;
                }
                KeyCode::Enter => {
                    let target = target.clone();
                    let text = line.text.trim().to_string();
                    self.edit = None;
                    return submit_edit(target, text);
                }
                other => line.on_key(other),
            }
            return None;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_by(&rows, -1),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(&rows, 1),
            KeyCode::Enter => return self.activate(&rows, snap),
            KeyCode::Char('e') => self.start_rename(&rows, snap),
            KeyCode::Char('d') => return self.delete(&rows, snap),
            KeyCode::Char('n') => self.start_new(&rows),
            _ => {}
        }
        None
    }

    pub fn on_paste(&mut self, text: String) {
        if let Some((_, line)) = self.edit.as_mut() {
            line.paste(&text);
        }
    }

    fn move_by(&mut self, rows: &[Row], dir: i16) {
        if rows.is_empty() {
            return;
        }
        let len = rows.len() as i16;
        for _ in 0..len {
            let next = (self.cursor as i16 + dir).rem_euclid(len) as usize;
            self.cursor = next;
            if rows[self.cursor].selectable() {
                break;
            }
        }
    }

    fn activate(&mut self, rows: &[Row], snap: &Snapshot) -> Option<DbOp> {
        let row = rows.get(self.cursor)?;
        match row {
            Row::Provider { id, .. } => Some(DbOp::ActivateProvider(*id)),
            Row::Model { id, .. } => Some(DbOp::ActivateModel(*id)),
            Row::Kind { provider_id, kind } => Some(DbOp::SetKind {
                id: *provider_id,
                kind: next_kind(kind).into(),
            }),
            Row::ApiKey { provider_id, .. } => {
                self.edit = Some((EditTarget::ApiKey { id: *provider_id }, LineEdit::default()));
                None
            }
            Row::BaseUrl {
                provider_id, url, ..
            } => {
                let initial = if url == "—" {
                    String::new()
                } else {
                    url.clone()
                };
                self.edit = Some((
                    EditTarget::BaseUrl { id: *provider_id },
                    LineEdit::from(initial),
                ));
                None
            }
            Row::System { .. } => {
                self.edit_system(&snap.system);
                None
            }
            Row::Thinking { on } => Some(DbOp::SetThinking(!on)),
            Row::ToolsEnabled { on } => Some(DbOp::SetToolsEnabled(!on)),
            Row::NewProvider => {
                self.edit = Some((EditTarget::NewProvider, LineEdit::default()));
                None
            }
            Row::NewModel { provider_id } => {
                self.edit = Some((
                    EditTarget::NewModel {
                        provider_id: *provider_id,
                    },
                    LineEdit::default(),
                ));
                None
            }
            Row::Header(_) => None,
        }
    }

    pub fn edit_system(&mut self, current: &str) {
        self.edit = Some((EditTarget::System, LineEdit::from(current)));
    }

    fn start_rename(&mut self, rows: &[Row], snap: &Snapshot) {
        match rows.get(self.cursor) {
            Some(Row::Provider { id, name, .. }) => {
                self.edit = Some((
                    EditTarget::RenameProvider { id: *id },
                    LineEdit::from(name.clone()),
                ));
            }
            Some(Row::Model { id, name, .. }) => {
                self.edit = Some((
                    EditTarget::RenameModel { id: *id },
                    LineEdit::from(name.clone()),
                ));
            }
            Some(Row::System { .. }) => self.edit_system(&snap.system),
            _ => {}
        }
    }

    fn start_new(&mut self, rows: &[Row]) {
        match rows.get(self.cursor) {
            Some(Row::Provider { .. } | Row::NewProvider) => {
                self.edit = Some((EditTarget::NewProvider, LineEdit::default()));
            }
            Some(Row::Model { .. } | Row::NewModel { .. }) => {
                if let Some(Row::NewModel { provider_id }) =
                    rows.iter().find(|r| matches!(r, Row::NewModel { .. }))
                {
                    self.edit = Some((
                        EditTarget::NewModel {
                            provider_id: *provider_id,
                        },
                        LineEdit::default(),
                    ));
                }
            }
            _ => {}
        }
    }

    fn delete(&self, rows: &[Row], snap: &Snapshot) -> Option<DbOp> {
        match rows.get(self.cursor) {
            Some(Row::Provider { id, .. }) if snap.providers.len() > 1 => {
                Some(DbOp::DeleteProvider(*id))
            }
            Some(Row::Model { id, .. }) => Some(DbOp::DeleteModel(*id)),
            _ => None,
        }
    }
}

fn submit_edit(target: EditTarget, text: String) -> Option<DbOp> {
    if text.is_empty()
        && !matches!(
            target,
            EditTarget::ApiKey { .. } | EditTarget::BaseUrl { .. } | EditTarget::System
        )
    {
        return None;
    }
    match target {
        EditTarget::System => Some(DbOp::SetSystem(text)),
        EditTarget::ApiKey { id } => Some(DbOp::SetApiKey { id, api_key: text }),
        EditTarget::BaseUrl { id } => Some(DbOp::SetBaseUrl { id, base_url: text }),
        EditTarget::NewProvider => Some(DbOp::NewProvider { name: text }),
        EditTarget::NewModel { provider_id } => Some(DbOp::NewModel {
            provider_id,
            name: text,
        }),
        EditTarget::RenameProvider { id } => Some(DbOp::RenameProvider { id, name: text }),
        EditTarget::RenameModel { id } => Some(DbOp::RenameModel { id, name: text }),
    }
}

fn next_kind(kind: &str) -> &'static str {
    match ProviderId::parse(kind).unwrap_or(ProviderId::Grok) {
        ProviderId::Grok => "gpt",
        ProviderId::Gpt => "ollama",
        ProviderId::Ollama => "claude",
        ProviderId::Claude => "codex",
        ProviderId::Codex => "grok",
    }
}

fn key_status(provider: &ira_store::ProviderRow) -> String {
    if provider
        .api_key
        .as_ref()
        .is_some_and(|k| !k.trim().is_empty())
    {
        return "en bbdd".into();
    }
    if let Ok(kind) = ProviderId::parse(&provider.kind) {
        if kind == ProviderId::Codex {
            return "falta OAuth".into();
        }
        if let Some(var) = kind.env_key() {
            if std::env::var(var).is_ok_and(|v| !v.trim().is_empty()) {
                return "env".into();
            }
            return "falta".into();
        }
    }
    "—".into()
}

fn preview(text: &str, max: usize) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let trimmed: String = flat.chars().take(max.saturating_sub(1)).collect();
    format!("{trimmed}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ira_store::stub_snapshot;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn enter_on_model_activates() {
        let snap = stub_snapshot("grok", "grok-4.6", "sé breve");
        let mut st = SettingsState::new();
        let rows = SettingsState::rows(&snap);
        st.cursor = rows
            .iter()
            .position(|r| matches!(r, Row::Model { .. }))
            .unwrap();
        let op = st.on_key(key(KeyCode::Enter), &snap).unwrap();
        assert!(matches!(op, DbOp::ActivateModel(_)));
    }

    #[test]
    fn enter_on_kind_cycles() {
        let snap = stub_snapshot("grok", "grok-4.6", "x");
        let mut st = SettingsState::new();
        let rows = SettingsState::rows(&snap);
        st.cursor = rows
            .iter()
            .position(|r| matches!(r, Row::Kind { .. }))
            .unwrap();
        let op = st.on_key(key(KeyCode::Enter), &snap).unwrap();
        match op {
            DbOp::SetKind { kind, .. } => assert_eq!(kind, "gpt"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn new_model_saves_on_enter() {
        let snap = stub_snapshot("grok", "grok-4.6", "x");
        let mut st = SettingsState::new();
        let rows = SettingsState::rows(&snap);
        st.cursor = rows
            .iter()
            .position(|r| matches!(r, Row::NewModel { .. }))
            .unwrap();
        assert!(st.on_key(key(KeyCode::Enter), &snap).is_none());
        assert!(st.edit.is_some());
        if let Some((_, line)) = st.edit.as_mut() {
            line.paste("grok-4.3");
        }
        let op = st.on_key(key(KeyCode::Enter), &snap).unwrap();
        match op {
            DbOp::NewModel { name, .. } => assert_eq!(name, "grok-4.3"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn refuses_to_delete_last_provider() {
        let snap = stub_snapshot("grok", "grok-4.6", "x");
        let mut st = SettingsState::new();
        let rows = SettingsState::rows(&snap);
        st.cursor = rows
            .iter()
            .position(|r| matches!(r, Row::Provider { .. }))
            .unwrap();
        assert!(st.on_key(key(KeyCode::Char('d')), &snap).is_none());
    }
}
