# ira-store: Catalog Persistence

`ira-store` owns all PostgreSQL state: providers, models, STT/TTS/wake
engines, settings (including the active model and voice parameters), secrets,
database connections, host services, MCP servers, and conversation history.
The schema lives in `deploy/postgres/init.sql`; additive changes are versioned
under `deploy/postgres/migrations/` and tracked in `schema_migrations`.

## Snapshot

`Snapshot` is an in-memory copy of the catalog and settings, including
internal provider credentials.

- `store::connect(url)` opens the pool; `store::migrate(pool)` applies the
  schema and migrations.
- `store::load(pool)` returns a fresh `Snapshot`.
- `Snapshot::client()` builds the active model's `ira-llm` client without
  network I/O; `store::client_with_pool(snap, pool)` is the shared entry used
  by TUI, API, and daemon.
- `snap.active_model()`, `snap.engine(role)`, `snap.reasoning_effort()` expose
  selected catalog rows; `apply_reasoning(&mut ChatRequest)` copies the
  thinking/effort settings into a request.
- Conversations are loaded separately (`ensure_local`, `context_messages`,
  `append_message`) — not inside `Snapshot`.

## DbOp — the edit contract

Every catalog mutation is a `DbOp` variant. `store::apply(pool, op)` persists
one operation and reloads the catalog, so callers always render from the
post-write state. Variants (see `crates/ira-store/src/lib.rs`):

| Group | Operations |
| --- | --- |
| Activation | `ActivateProvider`, `ActivateModel` |
| Providers | `NewProvider`, `RenameProvider`, `DeleteProvider`, `SetKind`, `SetApiKey`, `SetBaseUrl` |
| Models | `NewModel`, `RenameModel`, `DeleteModel`, `SetModelEffort` |
| Prompts | `SetSystem`, `SetVoiceSystem` |
| Voice | `SetEngine`, `SetVoiceAudio`, `SetVad`, `SetSttLanguage` |
| Chat behavior | `SetThinking`, `SetToolsEnabled`, `SetToolsMutate` |
| Telegram | `SetTelegram` (token + allowlist) |
| Secrets | `SetSecret` |

`DbOp` serializes with a snake_case `op` tag. The frontend `Op` union in
`desktop/src/types.ts` mirrors these names 1:1 — see
[Shared DTO and Op Contract](#/dto-contract).

## Secrets and encryption

- The `secrets` table stores `key/value` pairs; `apply_secrets_to_env(pool)`
  exports them into the process environment at startup (without overwriting
  exported variables).
- Provider API keys and database passwords are stored encrypted:
  `api_key_ciphertext`/`api_key_nonce` on `providers`,
  `password_ciphertext`/`password_nonce` on `database_connections`. The
  master key comes from `IRA_MASTER_KEY`; when unset, Ira creates
  `.ira/master.key` on first use and reuses it.
- Empty API keys fall back to provider environment variables through
  `ira-llm` (e.g. `XAI_API_KEY`).
- Desktop DTOs expose credential status rather than values; never pass raw
  rows across the frontend boundary.

## Ollama discovery

`sync_ollama_providers(pool)` discovers models from configured Ollama
providers (native `/api/tags` listing). An unavailable provider is skipped;
database errors still propagate. The TUI and API call it lazily before a turn
when the active provider is Ollama.

## Conversations

- `conversations` rows carry a `channel` (`local`, `telegram`, `voice`) with
  partial unique indexes: one live voice conversation, one live row per
  Telegram `external_id`.
- `messages` rows carry `role` (`user`, `assistant`, `tool`, `error`),
  optional `tool_call_id`, `name`, `tool_calls` JSONB, and the `model_id`
  that produced assistant text.
- `context_messages(pool, id, limit)` returns the recent window used to build
  a `ChatRequest` history.
- `NewMessage::user/assistant/error` are the constructors used by every
  surface; errors are persisted as `role='error'` messages so history shows
  failed turns.

## Failure semantics

- Connection failures at daemon startup surface a Chinese-commented hint in
  the error message: start the database with `docker compose up -d`.
- `apply` returns the new `Snapshot` on success; SQL errors propagate to the
  caller (TUI shows them; HTTP maps to `{"error": ...}` with 500).
- Voice transcripts that fail to persist log a warning (`voz no guardada`)
  without breaking the session — persistence is fire-and-forget on the
  daemon's `ira-events` thread.
