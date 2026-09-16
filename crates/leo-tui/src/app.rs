use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use leo_llm::{ChatMessage, ChatRequest, ChatResponse, LlmError};

use crate::input::LineEdit;
use crate::settings::SettingsState;
use crate::slash::{self, SlashItem};
use leo_store::{DbOp, MessageRow, Snapshot};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    User,
    Leo,
    Error,
}

#[derive(Debug, Clone)]
pub struct Bubble {
    pub kind: Kind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Chat,
    Settings,
}

pub struct App {
    pub screen: Screen,
    pub input: LineEdit,
    pub bubbles: Vec<Bubble>,
    pub history: Vec<ChatMessage>,
    pub scroll: u16,
    pub follow: bool,
    pub busy: bool,
    pub should_quit: bool,
    pub snapshot: Snapshot,
    pub settings: SettingsState,
    pub slash_cursor: usize,
    pub slash_pick: Option<uuid::Uuid>,
    pub conversation_id: Uuid,
    pending_chat: Option<ChatRequest>,
    pending_db: Option<DbOp>,
    pending_new_conversation: bool,
}

impl App {
    pub fn new(snapshot: Snapshot) -> Self {
        Self::from_store(snapshot, Uuid::nil(), Vec::new())
    }

    pub fn from_store(
        snapshot: Snapshot,
        conversation_id: Uuid,
        messages: Vec<MessageRow>,
    ) -> Self {
        let mut app = Self {
            screen: Screen::Chat,
            input: LineEdit::default(),
            bubbles: Vec::new(),
            history: Vec::new(),
            scroll: 0,
            follow: true,
            busy: false,
            should_quit: false,
            snapshot,
            settings: SettingsState::new(),
            slash_cursor: 0,
            slash_pick: None,
            conversation_id,
            pending_chat: None,
            pending_db: None,
            pending_new_conversation: false,
        };
        for m in messages {
            match m.role.as_str() {
                "user" => {
                    app.bubbles.push(Bubble {
                        kind: Kind::User,
                        text: m.content.clone(),
                    });
                    app.history.push(ChatMessage::user(m.content));
                }
                "assistant" => {
                    app.bubbles.push(Bubble {
                        kind: Kind::Leo,
                        text: m.content.clone(),
                    });
                    app.history.push(ChatMessage::assistant(m.content));
                }
                "error" => app.bubbles.push(Bubble {
                    kind: Kind::Error,
                    text: m.content,
                }),
                _ => {}
            }
        }
        app
    }

    pub fn slash_open(&self) -> bool {
        self.screen == Screen::Chat && slash::active(&self.input.text)
    }

    pub fn slash_picking(&self) -> bool {
        self.slash_pick.is_some()
    }

    pub fn slash_items(&self) -> Vec<SlashItem> {
        slash::items(&self.snapshot, &self.input.text, self.slash_pick)
    }

    pub fn provider_label(&self) -> String {
        self.snapshot
            .active_provider()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "—".into())
    }

    pub fn model_label(&self) -> String {
        self.snapshot
            .active_model()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "—".into())
    }

    pub fn system(&self) -> &str {
        &self.snapshot.system
    }

    pub fn take_pending_chat(&mut self) -> Option<ChatRequest> {
        self.pending_chat.take()
    }

    pub fn take_pending_db(&mut self) -> Option<DbOp> {
        self.pending_db.take()
    }

    pub fn take_new_conversation(&mut self) -> bool {
        std::mem::take(&mut self.pending_new_conversation)
    }

    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
        let rows = SettingsState::rows(&self.snapshot);
        self.settings.clamp(&rows);
    }

    pub fn on_db_err(&mut self, err: impl ToString) {
        self.push_error(err);
        self.screen = Screen::Chat;
    }

    pub fn push_error(&mut self, err: impl ToString) {
        self.bubbles.push(Bubble {
            kind: Kind::Error,
            text: err.to_string(),
        });
        self.follow = true;
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.should_quit = true;
            return;
        }

        if self.slash_open() {
            self.on_slash_key(key);
            return;
        }

        if key.code == KeyCode::Tab && self.settings.edit.is_none() {
            self.screen = match self.screen {
                Screen::Chat => Screen::Settings,
                Screen::Settings => Screen::Chat,
            };
            return;
        }

        match self.screen {
            Screen::Chat => self.on_chat_key(key),
            Screen::Settings => {
                if key.code == KeyCode::Esc && self.settings.edit.is_none() {
                    self.screen = Screen::Chat;
                    return;
                }
                if let Some(op) = self.settings.on_key(key, &self.snapshot) {
                    self.pending_db = Some(op);
                }
            }
        }
    }

    pub fn on_paste(&mut self, text: String) {
        match self.screen {
            Screen::Chat => {
                self.input.paste(&text);
                self.clamp_slash();
            }
            Screen::Settings => self.settings.on_paste(text),
        }
    }

    pub fn on_reply(&mut self, result: Result<ChatResponse, LlmError>) {
        self.busy = false;
        match result {
            Ok(resp) => {
                self.bubbles.push(Bubble {
                    kind: Kind::Leo,
                    text: resp.text.clone(),
                });
                self.history.push(ChatMessage::assistant(resp.text));
            }
            Err(err) => {
                self.bubbles.push(Bubble {
                    kind: Kind::Error,
                    text: err.to_string(),
                });
            }
        }
        self.follow = true;
    }

    pub fn clamp_scroll(&mut self, max_scroll: u16) {
        if self.follow {
            self.scroll = max_scroll;
        } else {
            self.scroll = self.scroll.min(max_scroll);
        }
    }

    fn on_chat_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Enter => self.submit(),
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            KeyCode::PageUp => {
                self.follow = false;
                self.scroll = self.scroll.saturating_sub(8);
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(8);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.clear();
            }
            other => {
                self.input.on_key(other);
                if slash::active(&self.input.text) {
                    self.clamp_slash();
                }
            }
        }
    }

    fn on_slash_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.slash_pick.take().is_some() {
                    self.input = LineEdit::from("/providers");
                    self.slash_cursor = 0;
                    self.clamp_slash();
                } else {
                    self.input.clear();
                }
            }
            KeyCode::Enter | KeyCode::Tab => self.apply_slash(),
            KeyCode::Up => self.slash_move(-1),
            KeyCode::Down => self.slash_move(1),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.slash_pick = None;
                self.input.clear();
            }
            other => {
                self.input.on_key(other);
                self.clamp_slash();
            }
        }
    }

    fn slash_move(&mut self, dir: i16) {
        let items = self.slash_items();
        self.slash_cursor = slash::step(&items, self.slash_cursor, dir);
    }

    fn clamp_slash(&mut self) {
        if self.slash_pick.is_some() && !slash::is_providers_cmd(&self.input.text) {
            self.slash_pick = None;
        }
        let items = self.slash_items();
        if items.is_empty() {
            self.slash_cursor = 0;
            return;
        }
        if self.slash_cursor >= items.len() || !items[self.slash_cursor].selectable() {
            self.slash_cursor = slash::first_selectable(&items).unwrap_or(0);
        }
    }

    fn apply_slash(&mut self) {
        self.clamp_slash();
        let items = self.slash_items();
        let Some(item) = items.get(self.slash_cursor).cloned() else {
            self.submit();
            return;
        };
        if !item.selectable() {
            return;
        }
        match item {
            SlashItem::Command { key: "model", .. } => {
                self.slash_pick = None;
                self.input = LineEdit::from("/model");
                self.slash_cursor = 0;
                self.clamp_slash();
            }
            SlashItem::Command {
                key: "providers", ..
            } => {
                self.slash_pick = None;
                self.input = LineEdit::from("/providers");
                self.slash_cursor = 0;
                self.clamp_slash();
            }
            SlashItem::Provider { id, .. } => {
                self.slash_pick = Some(id);
                self.input = LineEdit::from("/providers");
                self.slash_cursor = 0;
                self.clamp_slash();
            }
            SlashItem::Model { id, .. } => {
                self.input.clear();
                self.slash_pick = None;
                self.slash_cursor = 0;
                self.pending_db = Some(DbOp::ActivateModel(id));
            }
            SlashItem::Config => {
                self.close_slash();
                self.screen = Screen::Settings;
            }
            SlashItem::System => {
                self.close_slash();
                self.screen = Screen::Settings;
                self.settings.edit_system(&self.snapshot.system);
            }
            SlashItem::Clear => {
                self.close_slash();
                self.bubbles.clear();
                self.history.clear();
                self.scroll = 0;
                self.follow = true;
                self.pending_new_conversation = true;
            }
            SlashItem::Command { .. } | SlashItem::Header(_) => {}
        }
    }

    fn close_slash(&mut self) {
        self.input.clear();
        self.slash_pick = None;
        self.slash_cursor = 0;
    }

    fn submit(&mut self) {
        if self.busy {
            return;
        }
        let text = self.input.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.input.clear();
        if self.snapshot.active_model().is_none() {
            self.bubbles.push(Bubble {
                kind: Kind::Error,
                text: "elige un modelo en config (tab)".into(),
            });
            self.follow = true;
            return;
        }
        self.bubbles.push(Bubble {
            kind: Kind::User,
            text: text.clone(),
        });
        self.history.push(ChatMessage::user(text));
        self.pending_chat = Some(ChatRequest::with_history(
            self.system(),
            self.history.clone(),
        ));
        self.busy = true;
        self.follow = true;
    }

    fn scroll_up(&mut self) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leo_llm::Role;
    use leo_store::stub_snapshot;

    fn app(system: &str) -> App {
        App::new(stub_snapshot("grok", "grok-4.6", system))
    }

    #[test]
    fn empty_input_does_not_send() {
        let mut app = app("sé breve");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.take_pending_chat().is_none());
        assert!(app.history.is_empty());
        assert!(!app.busy);
    }

    #[test]
    fn enter_queues_history_and_system() {
        let mut app = app("sé breve");
        app.input.paste("hola");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        let req = app.take_pending_chat().expect("request");
        assert!(app.busy);
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, Role::System);
        assert_eq!(req.messages[0].content, "sé breve");
        assert_eq!(req.messages[1].role, Role::User);
        assert_eq!(req.messages[1].content, "hola");
        assert!(app.input.text.is_empty());
    }

    #[test]
    fn reply_appends_assistant_not_errors() {
        let mut app = app("");
        app.history.push(ChatMessage::user("hola"));
        app.busy = true;
        app.on_reply(Ok(ChatResponse::new(
            leo_llm::ProviderId::Grok,
            "grok-4.6",
            "hey",
        )));
        assert!(!app.busy);
        assert_eq!(app.history.last().unwrap().role, Role::Assistant);
        assert_eq!(app.history.last().unwrap().content, "hey");

        app.busy = true;
        app.on_reply(Err(LlmError::Empty("grok")));
        assert_eq!(app.history.len(), 2);
        assert_eq!(app.bubbles.last().unwrap().kind, Kind::Error);
    }

    #[test]
    fn tab_toggles_settings() {
        let mut app = app("x");
        app.on_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.screen, Screen::Settings);
        app.on_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.screen, Screen::Chat);
        app.on_key(KeyEvent::from(KeyCode::Tab));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Chat);
    }

    #[test]
    fn slash_opens_and_esc_does_not_quit() {
        let mut app = app("x");
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        assert!(app.slash_open());
        assert!(!app.slash_items().is_empty());
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.slash_open());
        assert!(!app.should_quit);
        assert!(app.input.text.is_empty());
    }

    #[test]
    fn slash_enter_activates_model() {
        let mut app = app("x");
        app.input.paste("/model");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(
            app.take_pending_db(),
            Some(DbOp::ActivateModel(_))
        ));
        assert!(app.input.text.is_empty());
    }

    #[test]
    fn slash_tab_does_not_open_settings() {
        let mut app = app("x");
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        app.on_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.screen, Screen::Chat);
        assert_eq!(app.input.text, "/model");
    }

    #[test]
    fn slash_filters_then_picks_model() {
        let mut app = app("x");
        app.input.paste("/model grok-4.6");
        let items = app.slash_items();
        assert!(
            items
                .iter()
                .any(|i| matches!(i, SlashItem::Model { name, .. } if name == "grok-4.6"))
        );
        app.slash_cursor = items
            .iter()
            .position(|i| matches!(i, SlashItem::Model { name, .. } if name == "grok-4.6"))
            .unwrap();
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(
            app.take_pending_db(),
            Some(DbOp::ActivateModel(_))
        ));
    }

    #[test]
    fn slash_clear_requests_new_conversation() {
        let mut app = app("x");
        app.history.push(ChatMessage::user("hola"));
        app.input.paste("/clear");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.take_new_conversation());
        assert!(app.history.is_empty());
    }

    #[test]
    fn slash_provider_then_model() {
        let mut app = app("x");
        app.input.paste("/providers");
        let items = app.slash_items();
        app.slash_cursor = items
            .iter()
            .position(|i| matches!(i, SlashItem::Provider { name, .. } if name == "grok"))
            .unwrap();
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.slash_picking());
        assert!(
            app.slash_items()
                .iter()
                .any(|i| matches!(i, SlashItem::Model { name, .. } if name == "grok-4.6"))
        );
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(
            app.take_pending_db(),
            Some(DbOp::ActivateModel(_))
        ));
        assert!(!app.slash_picking());
    }
}
