use std::collections::HashMap;
use std::sync::Arc;

use ira_llm::ToolSpec;

use crate::catalog;
use crate::context::Context;
use crate::error::ToolError;
use crate::tool::{DynTool, Tool};

/// Where a registered tool executes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolProvider {
    /// Local function compiled into Ira.
    Native,
    /// Tool advertised by a connected MCP server.
    Mcp {
        /// `mcp_manager.id`.
        server_id: String,
        /// Name returned by `tools/list`, before prefixing.
        tool_name: String,
    },
}

/// Cloneable catalog with shared tools; an empty registry offers no tools.
#[derive(Clone, Default)]
pub struct Registry {
    ctx: Context,
    tools: Arc<Vec<DynTool>>,
    providers: Arc<HashMap<String, ToolProvider>>,
}

impl Registry {
    /// Starts a registry builder with the execution context shared by its tools.
    pub fn builder(ctx: Context) -> Builder {
        Builder {
            ctx,
            tools: Vec::new(),
            providers: HashMap::new(),
        }
    }

    /// Registers local tools and integrations enabled by environment settings.
    pub fn from_env() -> Self {
        let mut b = Self::builder(Context::from_env());
        catalog::register(&mut b);
        b.build()
    }

    /// Returns whether the registry contains no tools.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Returns model-facing specifications for all registered tools.
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|t| t.spec()).collect()
    }

    /// Returns registered tool names in insertion order.
    pub fn names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// Returns the executor for a registered tool name.
    pub fn provider(&self, name: &str) -> ToolProvider {
        self.providers
            .get(name)
            .cloned()
            .unwrap_or(ToolProvider::Native)
    }

    /// Invokes the first matching name; failures are returned as JSON content.
    pub async fn call(&self, name: &str, args: serde_json::Value) -> String {
        if let ToolProvider::Mcp {
            server_id,
            tool_name,
        } = self.provider(name)
        {
            return match ira_mcp::shared().call_tool(&server_id, &tool_name, args).await {
                Ok(value) => value.to_string(),
                Err(err) => ToolError::from_display(err).to_json(),
            };
        }
        match self.tools.iter().find(|t| t.name() == name) {
            Some(tool) => match tool.invoke(&self.ctx, args).await {
                Ok(s) => s,
                Err(err) => err.to_json(),
            },
            None => ToolError::Unknown(name.to_string()).to_json(),
        }
    }

    /// Copies the current tools so more can be added.
    pub fn builder_from(&self) -> Builder {
        Builder {
            ctx: self.ctx.clone(),
            tools: (*self.tools).clone(),
            providers: (*self.providers).clone(),
        }
    }

    /// Returns whether a tool with this name is already registered.
    pub fn has(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name() == name)
    }

    /// Drops tools that write, delete, or run commands. MCP servers stay.
    pub fn read_only(&self) -> Self {
        let tools = self
            .tools
            .iter()
            .filter(|tool| !crate::catalog::is_mutating(&tool.name()))
            .cloned()
            .collect();
        let providers = self
            .providers
            .iter()
            .filter(|(name, _)| !crate::catalog::is_mutating(name))
            .map(|(name, provider)| (name.clone(), provider.clone()))
            .collect();
        Self {
            ctx: self.ctx.clone(),
            tools: Arc::new(tools),
            providers: Arc::new(providers),
        }
    }
}

/// Incrementally constructs a [`Registry`] with one shared [`Context`].
pub struct Builder {
    ctx: Context,
    tools: Vec<DynTool>,
    providers: HashMap<String, ToolProvider>,
}

impl Builder {
    /// Adds an already type-erased tool to the registry.
    pub fn add(&mut self, tool: DynTool) {
        self.tools.push(tool);
    }

    /// Registers a tool and records which MCP server owns the remote name.
    pub fn add_mcp(&mut self, tool: DynTool, server_id: impl Into<String>, tool_name: impl Into<String>) {
        let name = tool.name();
        self.providers.insert(
            name,
            ToolProvider::Mcp {
                server_id: server_id.into(),
                tool_name: tool_name.into(),
            },
        );
        self.tools.push(tool);
    }

    /// Adapts and adds an asynchronous function with its model-facing specification.
    pub fn add_fn<F, Fut>(&mut self, spec: ToolSpec, f: F)
    where
        F: Fn(Context, serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<String, ToolError>> + Send + 'static,
    {
        self.add(DynTool::new(spec, f));
    }

    /// Finishes construction of the registry.
    pub fn build(self) -> Registry {
        Registry {
            ctx: self.ctx,
            tools: Arc::new(self.tools),
            providers: Arc::new(self.providers),
        }
    }
}
