use std::sync::Arc;

use ira_llm::ToolSpec;

use crate::catalog;
use crate::context::Context;
use crate::error::ToolError;
use crate::tool::{DynTool, Tool};

/// Cloneable catalog with shared tools; an empty registry offers no tools.
#[derive(Clone, Default)]
pub struct Registry {
    ctx: Context,
    tools: Arc<Vec<DynTool>>,
}

impl Registry {
    /// Starts a registry builder with the execution context shared by its tools.
    pub fn builder(ctx: Context) -> Builder {
        Builder {
            ctx,
            tools: Vec::new(),
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

    /// Invokes the first matching name; failures are returned as JSON content.
    pub async fn call(&self, name: &str, args: serde_json::Value) -> String {
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
        }
    }

    /// Returns whether a tool with this name is already registered.
    pub fn has(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name() == name)
    }
}

/// Incrementally constructs a [`Registry`] with one shared [`Context`].
pub struct Builder {
    ctx: Context,
    tools: Vec<DynTool>,
}

impl Builder {
    /// Adds an already type-erased tool to the registry.
    pub fn add(&mut self, tool: DynTool) {
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
        }
    }
}
