# PostgreSQL Schema

The catalog lives in PostgreSQL 16 (Compose service on port `5439`,
`postgres://ira:ira@127.0.0.1:5439/ira`). Base schema:
`deploy/postgres/init.sql`; additive changes under
`deploy/postgres/migrations/` recorded in `schema_migrations`.

## Tables

### providers

| Column | Notes |
| --- | --- |
| `id UUID PK` | Well-known seeded IDs for grok/gpt/ollama/claude/codex. |
| `name TEXT UNIQUE` | Display + lookup name. |
| `kind TEXT` | Adapter selector: `grok`, `gpt`, `ollama`, `claude`, `codex`. |
| `base_url TEXT` | Provider endpoint (e.g. `http://127.0.0.1:11434` for Ollama). |
| `api_key TEXT` | Legacy/plaintext slot; prefer the encrypted columns. |
| `api_key_ciphertext BYTEA`, `api_key_nonce BYTEA` | Encrypted key (master key `IRA_MASTER_KEY` or `.ira/master.key`). |
| `created_at` | Audit. |

### models

`id`, `provider_id → providers ON DELETE CASCADE`, `name`
(unique per provider), `effort CHECK IN ('low','medium','high','xhigh')`.
`effort` feeds the reasoning level applied to chat requests.

### engines

Voice engines: `role CHECK IN ('stt','tts','wake')`, `kind`, `name`
(unique per role), optional `provider_id`, `config JSONB` (e.g.
`{"model": "/path/m.onnx"}` for wake). `settings` reference the selected
engine per role.

### settings (singleton, `id = 1`)

Active model and engine pointers, `system_prompt`, `voice_system_prompt`,
audio device names (`audio_source`/`audio_sink`, default
`@DEFAULT_SOURCE@`/`@DEFAULT_SINK@`), VAD tuning (`vad_hangover_ms` default
500), barge-in (`barge_in` bool + `barge_in_rms` 0.035), `stt_language`
(`es`), `thinking`, `tools_enabled`, `tools_mutate`,
`active_conversation_id`, `telegram_token`, `telegram_allow_users BIGINT[]`.

### conversations / messages

- `conversations`: `channel CHECK IN ('local','telegram','voice')`,
  `external_id`, `title`, `archived_at`. Partial unique indexes guarantee one
  live voice row and one live row per Telegram `external_id`.
- `messages`: `role CHECK IN ('user','assistant','tool','error')`,
  `content`, `tool_call_id`, `name`, `tool_calls JSONB`, `model_id` (assistant
  rows record which model answered), `created_at`; indexed by
  `(conversation_id, created_at)`.

### secrets

`key TEXT PK`, `value TEXT`, `updated_at`. Exported into the process
environment at startup (`apply_secrets_to_env`) without overwriting exported
variables.

### database_connections

Registered external databases for the MCP Toolbox bridge: host/port/database/
user, `ssl_mode CHECK IN ('disable','prefer','require','verify-ca',
'verify-full')`, `enabled`, encrypted `password_ciphertext`/`password_nonce`
(CHECK keeps them paired), last-test outcome (`last_test_ok`,
`last_test_error`, `last_tested_at`). The web panel renders enabled rows
into `.ira/toolbox` for Toolbox hot reload.

### host_services

The service board definition: `id TEXT PK` (Compose service name), `name`,
`required`, `position`. Seeded with `postgres`, `toolbox`, `ira-realtime`,
`colibri`.

### mcp_manager

External MCP servers: `transport CHECK IN ('stdio','streamable_http')` with
paired CHECKs (`stdio` ⇒ `command`, `streamable_http` ⇒ `url`), `args JSONB`,
`env JSONB`, `enabled`. Loaded at tool-registry build time by
`ira_tools::attach_configured`.

## Seeded catalog

Fixed UUIDs seed providers grok (`https://api.x.ai/v1`), gpt
(`https://api.openai.com/v1`), ollama (`http://127.0.0.1:11434`), claude
(`https://api.anthropic.com/v1`), codex (`https://chatgpt.com/backend-api/codex`)
with a few default models (grok default active), the STT/TTS/wake engine
rows, and the singleton `settings` row with Spanish system prompts.

## Migration rules

- `init.sql` is the current-state baseline for fresh databases.
- Additive changes ship as numbered files under
  `deploy/postgres/migrations/`, recorded in `schema_migrations`
  (`version INT PK`, `applied_at`).
- `store::migrate` runs at every process start; migrations must be
  idempotent with the seeded rows (`ON CONFLICT DO NOTHING`).

## Operational notes

- Database users exposed to the model (via Toolbox) must have **read-only**
  grants; the session default alone is not an authorization boundary.
- Use `host.docker.internal` inside containers for PostgreSQL on the Docker
  host.
- Set `IRA_UID`/`IRA_GID` when Ira writes runtime files as a non-`1000:1000`
  user.
