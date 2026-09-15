# Architecture and crate map

Leo is a Cargo workspace containing `crates/leo-*`, integration crates under
`crates/tools/*`, and the Tauri backend in `desktop/src-tauri`.

## Two execution paths

```text
Chat
  TUI / Telegram / React → Tauri
          │                 │
          ├─────────────────┘
          ▼
      leo-store → PostgreSQL catalog
          │
          ▼
      leo-tools → integration crates
          │
          ▼
       leo-llm → provider HTTP API

Voice
  leo-ctl / Tauri → leo-ipc → leo-daemon
                                 │
                             leo-core
                                 │
             capture → VAD → STT → LLM → TTS → playback
```

Chat surfaces share persistent provider/model settings, not conversation history.
The voice daemon reads TOML configuration and does not use `leo-store` or the
`leo-tools` chat loop.

## Applications

| Crate / directory | Entry point | Responsibility |
| --- | --- | --- |
| `leo-tui` | `leo` | Ratatui chat and catalog editor. `app` owns state; `input` and `slash` route input; `settings` and `ui` handle editing and rendering. |
| `leo-telegram` | `leo-telegram` | Long polling, allowlist enforcement, per-session history, and shared chat tools. Library routing is separate from `tg` HTTP transport. |
| `desktop/` | `npm run tauri dev` | React shell and native commands for chat, catalog editing, and voice control. |
| `leo-daemon` | `leo-daemon` | Loads voice settings, builds providers, starts the engine, and serves Unix IPC. |
| `leo-ctl` | `leo-ctl` | Sends one voice command and prints the daemon response. |

## Shared chat layer

### `leo-store`: catalog persistence

PostgreSQL stores providers, model names, active selection, and system prompt.
The schema and seed data live in `deploy/postgres/init.sql`.

- `Snapshot` is an in-memory catalog copy, including internal provider credentials.
- `Snapshot::client()` builds the active model's client without network I/O.
- `DbOp` represents edits; `apply` persists one operation and reloads the catalog.
- `sync_ollama_providers` discovers models from configured Ollama providers.
  An unavailable provider is skipped; database errors can still propagate.
- Empty API keys fall back to provider environment variables through `leo-llm`.

Desktop DTOs expose credential status rather than actual key values. Do not pass
internal database rows directly across the frontend boundary.

### `leo-llm`: provider transport

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

### `leo-tools`: tool orchestration

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
`run(...)` (typed async execution). `leo-tools/src/catalog.rs` adapts raw JSON
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
| `notion`, `spotify` | None | Placeholder crates, not registered. |

## Voice layer

### `leo-core`: state machine and engine

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
| `leo-audio` | Pulse capture/playback at 48 kHz; capture emits 16 kHz mono frames. An eight-frame capture queue drops new frames when full. |
| `leo-vad` | WebRTC VAD at 16 kHz with minimum speech and silence hangover thresholds. Detector stays on its owning thread. |
| `leo-wake` | `WakeSpotter` extension point; the loader currently returns `NoopWake`. |
| `leo-stt` | Synchronous `SttEngine`; Grok uploads mono PCM16 WAV, while `NullStt` returns no transcript. |
| `leo-tts` | Synchronous `TtsEngine` returns mono PCM with a sample rate; `NullTts` generates a tone. |

`GrokStt` and the daemon's `BlockingLlm` bridge async HTTP with
`tokio::runtime::Handle::block_on`. Call them from blocking threads while the
runtime remains active, not from async tasks. Voice LLM requests contain the
current user input and system prompt rather than persistent chat history.

### `leo-ipc`: daemon control

The Unix socket lives at `$XDG_RUNTIME_DIR/leo-ai.sock`, falling back to
`/tmp/leo-ai.sock`. Each exchange contains a newline-terminated JSON request and
response. Requests use a `cmd` discriminator:

```json
{"cmd":"speak","text":"Hello"}
```

Commands are `status`, `listen`, `stop`, `speak`, and `shutdown`. The client does
not impose a timeout. Server binding removes the existing socket entry, so the
caller must ensure another daemon is not already using it.

## Desktop boundary

- `src/App.tsx`: conversation state, display bubbles, voice polling, theme, and catalog visibility.
- `src/Catalog.tsx`: local form drafts and catalog operations.
- `src/api.ts`: Tauri invocation or browser-preview mocks.
- `src/types.ts`: frontend DTOs and tagged operations mirrored by Rust.
- `src/theme.ts`: saved theme preference and root CSS selector.
- `src-tauri/src/lib.rs`: native commands, database access, tool-enabled chat, and voice IPC.

Keep frontend field names, operation tags, and native DTOs synchronized. Browser
preview validates interaction and layout, not live provider, database, or IPC behavior.

## Extending the code

- **New tool:** implement its schema and typed operation in an integration crate,
  then register argument conversion in `leo-tools/src/catalog.rs`.
- **New LLM provider:** extend provider identity/defaults, client dispatch, and a
  protocol adapter; add offline payload and response tests.
- **New speech backend:** implement the relevant voice trait and wire it in the
  daemon, keeping playback in `leo-audio`.
- **New catalog operation:** update `DbOp`, persistence, and affected UI adapters;
  desktop also requires matching TypeScript and Rust operation variants.

See [README.md](README.md#development-checks) for build, documentation, and test commands.
