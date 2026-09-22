use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ira_llm::ToolSpec;

use crate::context::Context;
use crate::error::ToolError;

/// Sendable boxed future returned by tool implementations.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Contract connecting a model-visible function description to local execution.
pub trait Tool: Send + Sync {
    /// Name, description, and argument JSON Schema sent to the model.
    fn spec(&self) -> ToolSpec;

    /// Returns the model-facing tool name from [`Self::spec`].
    fn name(&self) -> String {
        self.spec().name
    }

    /// Executes JSON arguments and returns content for the model.
    ///
    /// Implementations validate arguments; the registry turns failures into
    /// JSON results so the model can respond to them.
    fn invoke(
        &self,
        ctx: &Context,
        args: serde_json::Value,
    ) -> BoxFuture<'_, Result<String, ToolError>>;
}

type RunFn = dyn Fn(Context, serde_json::Value) -> BoxFuture<'static, Result<String, ToolError>>
    + Send
    + Sync;

/// Adapts an async function into a tool with shared ownership through `Arc`.
#[derive(Clone)]
pub struct DynTool {
    spec: ToolSpec,
    run: Arc<RunFn>,
}

impl DynTool {
    /// Creates a tool from a specification and asynchronous execution function.
    pub fn new<F, Fut>(spec: ToolSpec, f: F) -> Self
    where
        F: Fn(Context, serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, ToolError>> + Send + 'static,
    {
        Self {
            spec,
            run: Arc::new(move |ctx, args| Box::pin(f(ctx, args))),
        }
    }
}

impl Tool for DynTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke(
        &self,
        ctx: &Context,
        args: serde_json::Value,
    ) -> BoxFuture<'_, Result<String, ToolError>> {
        (self.run)(ctx.clone(), args)
    }
}
