use ira_llm::ToolSpec;
use ira_mcp::{McpServerConfig, McpTransport};

use crate::error::stringify;
use crate::registry::Registry;
use crate::tool::DynTool;

/// MCP servers from `ira.json` (global, then project). Disabled entries are omitted.
pub fn file_servers() -> Vec<McpServerConfig> {
    ira_mcp::enabled_servers()
}

/// Registers every tool advertised by the given MCP servers.
///
/// A server that does not answer is skipped. Existing tool names are kept.
/// Tools are not stored in the server config; they come from `tools/list`.
pub async fn attach_mcp(base: Registry, endpoints: &[(&str, &str)]) -> Registry {
    let configs: Vec<McpServerConfig> = endpoints
        .iter()
        .map(|(id, url)| McpServerConfig::http((*id).to_string(), (*url).to_string()))
        .collect();
    attach_configured(base, &configs).await
}

/// Connects each [`McpServerConfig`] and registers the tools it advertises.
pub async fn attach_configured(base: Registry, configs: &[McpServerConfig]) -> Registry {
    if configs.is_empty() {
        return base;
    }
    let manager = ira_mcp::shared();
    let mut builder = base.builder_from();
    let mut added = Vec::new();
    for config in configs {
        if !config.enabled {
            continue;
        }
        let timeout = match config.transport {
            McpTransport::Stdio => std::time::Duration::from_secs(30),
            McpTransport::StreamableHttp => std::time::Duration::from_secs(5),
        };
        match tokio::time::timeout(timeout, manager.connect(config)).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::debug!("mcp {} no conecta: {err}", config.id);
                continue;
            }
            Err(_) => {
                tracing::debug!("mcp {} timeout al conectar", config.id);
                continue;
            }
        }
        let Ok(remote) = manager.list_tools(&config.id).await else {
            tracing::debug!("mcp {} no listó tools", config.id);
            continue;
        };
        tracing::info!("mcp {}: {} herramientas", config.id, remote.len());
        for tool in remote {
            let name = tool_name(&config.id, &tool.name);
            if base.has(&name) || added.iter().any(|seen| seen == &name) {
                continue;
            }
            added.push(name.clone());
            let spec = ToolSpec {
                name: name.clone(),
                description: tool.description,
                parameters: tool.parameters,
            };
            let remote_name = tool.name.clone();
            let server_id = config.id.clone();
            builder.add_mcp(
                DynTool::new(spec, move |_ctx, args| {
                    let server_id = server_id.clone();
                    let remote_name = remote_name.clone();
                    async move {
                        stringify(ira_mcp::shared().call_tool(&server_id, &remote_name, args).await)
                    }
                }),
                &config.id,
                tool.name,
            );
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
        assert_eq!(tool_name("projects-mcp", "create_task"), "projects_create_task");
        assert_eq!(tool_name("projects-mcp", "projects_list"), "projects_list");
    }
}
