# Architecture and crate map

Ira is a Cargo workspace containing `crates/ira-*`, integration crates under
`crates/tools/*`, and the Tauri backend in `desktop/src-tauri`.

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

Chat surfaces share the PostgreSQL catalog (providers, models, engines, settings)
and conversation history. The browser uses `ira-server` so it can reach Ollama
without CORS. Voice crates remain in the workspace but are not exposed in TUI,
Telegram, desktop, or the browser until that work is scheduled.

## Applications

| Crate / directory | Entry point | Responsibility |
| --- | --- | --- |
| `ira-tui` | `ira` | Ratatui chat and catalog editor. `app` owns state; `input` and `slash` route input; `settings` and `ui` handle editing and rendering. |
| `ira-telegram` | `ira-telegram` | Long polling, allowlist enforcement, per-session history, and shared chat tools. Library routing is separate from `tg` HTTP transport. |
| `desktop/` | `npm run tauri dev` | React shell and native commands for chat and catalog editing. |
| `ira-api` | library | Shared catalog DTOs and chat used by Tauri and `ira-server`. |
| `ira-server` | `ira-server` | HTTP `/api` for the browser and other machines. Talks to Ollama from the server process. |
| `ira-daemon` | `ira-daemon` | Voice process; deferred. Loads catalog and engines, serves Unix IPC. |
| `ira-ctl` | `ira-ctl` | Voice CLI; deferred. |

## Shared chat layer

### `ira-store`: catalog persistence

PostgreSQL stores providers, models, STT/TTS/wake engines, settings (including
the active model and voice parameters), secrets, and conversations.
The schema lives in `deploy/postgres/init.sql`; additive changes are versioned
under `deploy/postgres/migrations/`.

- `Snapshot` is an in-memory catalog and settings copy, including internal provider credentials.
- Conversations are loaded separately (`ensure_local`, `context_messages`, `append_message`).
- `Snapshot::client()` builds the active model's client without network I/O.
- `DbOp` represents edits; `apply` persists one operation and reloads the catalog.
- `sync_ollama_providers` discovers models from configured Ollama providers.
  An unavailable provider is skipped; database errors can still propagate.
- Empty API keys fall back to provider environment variables through `ira-llm`.

Desktop DTOs expose credential status rather than actual key values. Do not pass
internal database rows directly across the frontend boundary.

### `ira-llm`: provider transport

`Client` owns provider settings and a reusable HTTP client. `ChatRequest`,
`ChatResponse`, and tool types form the provider-independent contract.

| Adapter | Providers | Role |
| --- | --- | --- |
| `openai_compat` | Grok, GPT | OpenAI-compatible messages and tool calls. |
| `ollama` | Ollama | Native chat and model discovery. |
| `claude` | Claude | Anthropic messages and tool-use translation. |

`protocol` exposes payload builders and parsers for offline tests. The client
transports tool calls but never executes them. `load_dotenv` loads environment
defaults without overwriting exported variables.

### `ira-tools`: tool orchestration

1. `Registry::from_env()` captures the execution context and registers available tools.
2. `chat` supplies their schemas to the model.
3. `run` executes requested calls sequentially and appends results with matching call IDs.
4. The next model request includes those results until a final response is returned.

The loop permits eight model responses. If all still request tools, it returns
`LlmError::ToolLoop`. Provider failures abort the turn. Tool errors become JSON
content so the model can handle them. Invalid argument JSON currently becomes
an empty object before individual tool validation.

`Context::resolve` joins relative paths to the captured working directory. It
does not canonicalize paths or provide a filesystem sandbox.

### Integration crates

Operation modules generally pair `spec()` (model-facing JSON Schema) with
`run(...)` (typed async execution). `ira-tools/src/catalog.rs` adapts raw JSON
arguments, resolves paths, and registers these implementations.

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
| `callmebot` | WhatsApp text to the configured number (`whatsapp_send`) | `CALLMEBOT_PHONE` and `CALLMEBOT_APIKEY`. Send-only; no replies. |
| `db` | SQL and schema discovery through a local MCP Toolbox (`db_list_tools`, `db_invoke`, `db_execute_sql`, …) | Official container managed by Compose. `docker compose up -d postgres toolbox` and `MCP_TOOLBOX_URL=http://127.0.0.1:5000`. |
| `notion`, `spotify` | None | Placeholder crates, not registered. |

## Voice layer (deferred)

Not wired into chat surfaces. Crates stay for a later pass.

### `ira-core`: state machine and engine

```text
idle → listening → recording → transcribing → thinking → speaking
```

`Session` contains transition logic and accumulated utterance audio. It returns
`Action` values for side effects and `SessionEvent` values for observers.
`spawn_engine` runs those actions on a dedicated thread with injected providers.
Provider calls are synchronous, so commands wait while those calls are running.

VAD silence closes an utterance. Empty transcripts or replies return the session
to idle. During playback, configured barge-in detection can stop playback and
reopen listening. Frame-based limits assume 20 ms blocks.

### Audio and speech crates

| Crate | Contract and behavior |
| --- | --- |
| `ira-audio` | Pulse capture/playback at 48 kHz; capture emits 16 kHz mono frames. An eight-frame capture queue drops new frames when full. |
| `ira-vad` | WebRTC VAD at 16 kHz with minimum speech and silence hangover thresholds. Detector stays on its owning thread. |
| `ira-wake` | `WakeSpotter` extension point; the loader currently returns `NoopWake`. |
| `ira-stt` | Synchronous `SttEngine`; Grok uploads mono PCM16 WAV, while `NullStt` returns no transcript. |
| `ira-tts` | Synchronous `TtsEngine` returns mono PCM with a sample rate; `NullTts` generates a tone. |

`GrokStt` and the daemon's `BlockingLlm` bridge async HTTP with
`tokio::runtime::Handle::block_on`. Call them from blocking threads while the
runtime remains active, not from async tasks. Voice LLM requests contain the
current user input and system prompt rather than persistent chat history.

### `ira-ipc`: daemon control

The Unix socket lives at `$XDG_RUNTIME_DIR/ira-ai.sock`, falling back to
`/tmp/ira-ai.sock`. Each exchange contains a newline-terminated JSON request and
response. Requests use a `cmd` discriminator:

```json
{"cmd":"speak","text":"Hello"}
```

Commands are `status`, `listen`, `stop`, `speak`, and `shutdown`. The client does
not impose a timeout. Server binding removes the existing socket entry, so the
caller must ensure another daemon is not already using it.

## Desktop and HTTP boundary

- `src/App.tsx`: conversation state, display bubbles, theme, catalog, and host service start.
- `src/Catalog.tsx`: local form drafts and catalog operations.
- `src/api.ts`: Tauri invocation, or `fetch` to `ira-server` when not in the webview.
- `src/types.ts`: frontend DTOs and tagged operations mirrored by `ira-api`.
- `src/theme.ts`: saved theme preference and root CSS selector.
- `src-tauri/src/lib.rs`: thin Tauri commands over `ira_api::App`.
- `crates/ira-server`: HTTP `/api` over the same `App`. Default bind `127.0.0.1:8787`.
  `IRA_HTTP_BIND=0.0.0.0:8787` serves the LAN; that also exposes tools.
  `GET/POST /api/services` reports and starts Compose `postgres` + `toolbox`.

Keep frontend field names, operation tags, and `ira-api` DTOs synchronized.
The browser never calls Ollama; `ira-server` does.

## Extending the code

- **New tool:** implement its schema and typed operation in an integration crate,
  then register argument conversion in `ira-tools/src/catalog.rs`. Database
  tools go through `ira-tools-db` and the local MCP Toolbox container, not a
  direct `sqlx` connection from the model.
- **New LLM provider:** extend provider identity/defaults, client dispatch, and a
  protocol adapter; add offline payload and response tests.
- **New speech backend:** implement the relevant voice trait and wire it in the
  daemon, keeping playback in `ira-audio`.
- **New catalog operation:** update `DbOp`, persistence, and affected UI adapters;
  desktop and `ira-server` share `ira-api` operation variants and TypeScript types.

See [README.md](README.md#development-checks) for build, documentation, and test commands.
