# Tool Integration Crates

Integration crates live in `crates/tools/*` and follow the `spec()` +
`run(...)` pattern documented in
[ira-tools](#/tools). `ira-tools/src/catalog.rs` adapts JSON arguments,
resolves paths through `Context`, and registers implementations.

| Crate suffix | Operations | Registration requirements |
| --- | --- | --- |
| `files` | Read, write, list, search, copy, move, remove | Always registered. |
| `shell` | Commands and scripts | Always registered; timeout and captured output. |
| `system` | Processes, applications, URLs, notifications, clipboard | Always registered; host utilities determine availability. |
| `weather` | Current weather and forecast | Open-Meteo; no API key. |
| `appflowy` | Page creation, retrieval, search, update, deletion | AppFlowy server configuration and credentials. |
| `github` | Issues and pull requests | `GITHUB_TOKEN` or `GH_TOKEN`. |
| `google` | Calendars and events | `GOOGLE_ACCESS_TOKEN` or `GOOGLE_API_KEY`. |
| `home-assistant` | Entity states and service calls | Server URL and token. |
| `db` | SQL and schema discovery through local MCP Toolbox | `docker compose up -d toolbox` and `MCP_TOOLBOX_URL=http://127.0.0.1:5000`. |
| `notion`, `spotify` | None | Placeholder crates, not registered. |

## Behavioral notes

- **shell**: output is captured and bounded; long-running commands hit the
  configured timeout and the tool returns what it has.
- **system**: capability-dependent (e.g. notifications need a working
  notification service; clipboard depends on the session's utilities).
- **appflowy**: the client caches an auth token and retries once on `401`
  after refreshing; endpoints cover workspace, folder, and page operations.
- **github / google / home-assistant**: thin typed clients over host HTTP
  APIs; a missing credential means the tool is simply not registered, so the
  model never sees it.
- **db**: never connects directly — every call is proxied through the local
  Toolbox container so credentials stay outside the model's reach. The
  `ira-pgjson` CLI can dump any connection's tables (schema, keys, relations,
  rows) to JSON for offline inspection.

## Argument helpers

`ira-tools/src/args.rs` provides the shared validators used by every crate:
`require_str`, `opt_str`, and `opt_str_list`. Use them in new tools so
argument errors stay uniform (they become JSON error content the model can
read).
