use super::{Client, Config};

/// Colibri model execution engine.
#[derive(Clone, Debug)]
pub struct Engine {
    client: Client,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(config),
        }
    }

    pub fn from_client(client: Client) -> Self {
        Self { client }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}
