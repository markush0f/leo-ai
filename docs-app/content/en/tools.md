# ira-tools: Tool Orchestration

`ira-tools` registers model-facing tools and runs the model↔tool loop. Every
chat surface (TUI, Telegram, `ira-api`) uses the same registry and the same
`chat` entry point.

## Registry lifecycle

1. `Registry::from_env()` captures the execution context (working directory,
   environment) and registers the always-available tools.
2. `attach_configured(registry, &file_servers())` adds external MCP servers
   defined in the `mcp_manager` catalog table / config files (stdio or
   streamable-HTTP transports) through `ira-mcp`.
3. `registry.read_only()` yields the read-only subset used by the Telegram
   surface; `tools_enabled` / `tools_mutate` settings gate mutation tools.

## The chat loop

`ira_tools::chat(&client, request, &registry)` drives one user turn:

```text
send ChatRequest (+ tool schemas)
  └─ model returns tool calls?
       run them sequentially, append results with matching call IDs,
       send the next model request with those results
  └─ repeat until a final text response
```

- The loop permits **eight** model responses. If all eight still request
  tools, it returns `LlmError::ToolLoop`.
- Provider failures abort the turn immediately.
- Tool failures do **not** abort: the error becomes JSON content so the model
  can recover and explain.
- Invalid argument JSON currently becomes an empty object before individual
  tool validation.

## Operation pattern

Integration modules pair two functions:

- `spec()` — the model-facing JSON Schema for the tool.
- `run(ctx, args...)` — typed async execution.

`ira-tools/src/catalog.rs` adapts raw JSON arguments, resolves paths against
the captured context, and registers the implementations.

## Context and safety

`Context::resolve` joins relative paths to the captured working directory. It
does **not** canonicalize paths and provides **no** filesystem sandbox — tools
run with the user's permissions. Treat `tools_mutate` and the Telegram
read-only subset as convenience switches, not security boundaries.

## Database tools

SQL and schema discovery go through `ira-tools-db` and the **local** MCP
Toolbox container (`db_list_tools`, `db_invoke`, `db_execute_sql`, …), not a
direct `sqlx` connection from the model. Requirements:

```sh
docker compose up -d postgres toolbox
MCP_TOOLBOX_URL=http://127.0.0.1:5000
```

The web panel writes encrypted PostgreSQL credentials to the catalog and
renders enabled connections into `.ira/toolbox` for Toolbox hot reload.
Database users must have read-only grants — the session default alone is not
an authorization boundary. Use `host.docker.internal` for PostgreSQL on the
Docker host.
