use std::time::Duration;

use ira_llm::ToolSpec;

use crate::error::stringify;
use crate::registry::Registry;

/// Registers every tool advertised by the given MCP servers.
///
/// A server that does not answer is skipped. Existing tool names are kept.
pub async fn attach_mcp(base: Registry, endpoints: &[(&str, &str)]) -> Registry {
    if endpoints.is_empty() {
        return base;
    }
    let mut builder = base.builder_from();
    let mut added = Vec::new();
    for (id, url) in endpoints {
        let probe = ira_tools_kanban::Client::with_timeout(*url, Duration::from_secs(2));
        let Ok(remote) = probe.list_tools().await else {
            tracing::debug!("mcp {id} no responde en {url}");
            continue;
        };
        let client = ira_tools_kanban::Client::new(*url);
        tracing::info!("mcp {id}: {} herramientas", remote.len());
        for tool in remote {
            let name = tool_name(id, &tool.name);
            if base.has(&name) || added.iter().any(|seen| seen == &name) {
                continue;
            }
            added.push(name.clone());
            let spec = ToolSpec {
                name: name.clone(),
                description: tool.description,
                parameters: tool.parameters,
            };
            let remote_name = tool.name;
            let client = client.clone();
            builder.add_fn(spec, move |_ctx, args| {
                let client = client.clone();
                let remote_name = remote_name.clone();
                async move { stringify(client.invoke(&remote_name, args).await) }
            });
        }
    }
    builder.build()
}

fn tool_name(service_id: &str, remote: &str) -> String {
    let slug = service_id
        .trim_end_matches("-mcp")
        .replace('-', "_")
        .to_ascii_lowercase();
    let remote = remote.replace('-', "_");
    if remote.starts_with(&format!("{slug}_")) {
        remote
    } else {
        format!("{slug}_{remote}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_remote_tool_with_service_slug() {
        assert_eq!(tool_name("veritas-mcp", "create_task"), "veritas_create_task");
        assert_eq!(tool_name("veritas-mcp", "veritas_list"), "veritas_list");
    }
}
