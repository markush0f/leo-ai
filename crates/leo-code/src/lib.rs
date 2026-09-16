//! Integration with the `agent-code-lib` agent runtime.

use agent_code_lib::{config::Config, error::ConfigError, state::AppState};

/// Creates an agent state without reading configuration files or environment variables.
pub fn new_state() -> AppState {
    AppState::new(Config::default())
}

/// Loads layered agent-code configuration and creates a fresh session state.
pub fn load_state() -> Result<AppState, ConfigError> {
    Config::load().map(AppState::new)
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_state_starts_empty() {
        let state = super::new_state();

        assert!(state.history().is_empty());
        assert_eq!(state.turn_count, 0);
    }
}
