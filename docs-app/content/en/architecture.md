# Architecture Overview

Ira is a Cargo workspace containing `crates/ira-*`, integration tool crates
under `crates/tools/*`, Python/Node services under `services/`, and the Tauri
backend in `desktop/src-tauri`.

## Two execution paths

```text
Chat
  TUI / Telegram / React → Tauri
  React (browser) → ira-server
          │                 │
          ├─────────────────┘
          ▼
      ira-api → ira-store → PostgreSQL catalog
          │
          ▼
      ira-tools → integration crates
          │
          ▼
       ira-llm → provider HTTP API (Ollama, Grok, GPT, Claude)

Voice
  ira-ctl / Tauri → ira-ipc → ira-daemon
                                 │
                             ira-core
                                 │
             capture → VAD → STT → LLM → TTS → playback
```

Chat surfaces share the PostgreSQL catalog (providers, models, engines,
settings) and conversation history. The browser uses `ira-server` so it can
reach Ollama without CORS. Voice crates remain in the workspace but are not
exposed in TUI, Telegram, desktop, or the browser until that work is scheduled.

## Applications

| Crate / directory | Entry point | Responsibility |
| --- | --- | --- |
| `ira-tui` | `ira` | Ratatui chat and catalog editor. `app` owns state; `input` and `slash` route input; `settings` and `ui` handle editing and rendering. |
| `ira-telegram` | `ira-telegram` | Long polling, allowlist enforcement, per-session history, shared chat tools. Library routing is separate from the `tg` HTTP transport. |
| `desktop/` | `npm run desktop` | React shell and native commands for chat and catalog editing. |
| `ira-api` | library | Shared catalog DTOs and chat used by Tauri and `ira-server`. |
| `ira-server` | `ira-server` | HTTP `/api` for the browser and other machines. Talks to Ollama from the server process. |
| `ira-daemon` | `ira-daemon` | Voice process (deferred). Loads catalog and engines, serves Unix IPC. |
| `ira-ctl` | `ira-ctl` | Voice CLI (deferred). Talks to the daemon over the IPC socket only. |
| `ira-pgjson` | `ira-pgjson` | Exports PostgreSQL tables of any connection to JSON (schema, keys, relations, rows). |

## Shared layers

- **`ira-store`** — catalog persistence in PostgreSQL: `Snapshot` (in-memory
  copy), `DbOp` (edits), conversations, secrets. See
  [ira-store](#/store).
- **`ira-llm`** — provider transport: one `Client`, three protocol adapters.
  See [ira-llm](#/llm).
- **`ira-tools`** — tool registry and the model↔tool loop. See
  [ira-tools](#/tools).
- **`ira-core`** — voice session state machine. See
  [Voice layer](#/voice).
- **`ira-ipc`** — daemon control protocol. See [ira-ipc](#/ipc).
- **`ira-mcp` / `ira-code` / `ira-engine`** — MCP server client, code
  tooling, and engine adapters (e.g. the Colibri client) used by the tool
  layer and services.

## Services (`services/`)

| Service | Role |
| --- | --- |
| `ira-realtime` | Python realtime voice bridge (WebSocket, 16 kHz). Talks to `ira-server` over HTTP with the bearer token; provides browser voice outside the deferred daemon. Started by `scripts/start-ira-realtime.sh`. |
| `projects-api` | Node project/task API exposed through the gateway. |
| `gateway` | Caddy front for the services board on `127.0.0.1:8790`. |

## Desktop and HTTP boundary

- `src/App.tsx`: conversation state, bubbles, theme, catalog, host service start.
- `src/Catalog.tsx`: local form drafts and catalog operations.
- `src/api.ts`: Tauri invocation, or `fetch` to `ira-server` when not in the webview.
- `src/types.ts`: frontend DTOs and tagged operations mirrored by `ira-api`.
- `src-tauri/src/lib.rs`: thin Tauri commands over `ira_api::App`.
- `crates/ira-server`: HTTP `/api` over the same `App`. Default bind
  `127.0.0.1:8787`. `GET/POST /api/services` reports and starts Compose
  services.

Keep frontend field names, operation tags, and `ira-api` DTOs synchronized.
The browser never calls Ollama; `ira-server` does.

## Security boundaries

- Desktop DTOs expose credential *status*, never key values. Do not pass raw
  database rows across the frontend boundary.
- `ira-server` optionally guards `/api` with a bearer token
  (`IRA_HTTP_TOKEN`, else `~/.ira/http.token`); the Vite dev proxy injects it.
- Database access from the model goes through the **local** MCP Toolbox
  container with read-only grants, not a direct `sqlx` connection.
- `Context::resolve` in the tool layer joins relative paths but provides no
  filesystem sandbox — the tool layer runs with the user's permissions.
