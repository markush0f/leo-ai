/// Configuration required by the Colibri provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub endpoint: String,
    pub model: String,
}

impl Config {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
        }
    }
}
