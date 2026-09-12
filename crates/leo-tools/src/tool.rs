use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use leo_llm::ToolSpec;

use crate::context::Context;
use crate::error::ToolError;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    fn name(&self) -> String {
        self.spec().name
    }

    fn invoke(
        &self,
        ctx: &Context,
        args: serde_json::Value,
    ) -> BoxFuture<'_, Result<String, ToolError>>;
}

type RunFn = dyn Fn(Context, serde_json::Value) -> BoxFuture<'static, Result<String, ToolError>>
    + Send
    + Sync;

#[derive(Clone)]
pub struct DynTool {
    spec: ToolSpec,
    run: Arc<RunFn>,
}

impl DynTool {
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
