# Extending Ira

## New tool

1. Create `crates/tools/ira-tools-<name>` with one module per operation, each
   pairing `spec()` (model-facing JSON Schema) with `run(...)` (typed async
   execution). Use the `args` helpers (`require_str`, `opt_str`,
   `opt_str_list`) so argument errors stay uniform.
2. Adapt and register in `ira-tools/src/catalog.rs`: convert JSON arguments,
   resolve paths through `Context`, gate on credentials (an unconfigured tool
   should not register at all rather than fail at call time).
3. Database-touching tools must go through `ira-tools-db` and the local MCP
   Toolbox container — never a direct `sqlx` connection from the model.
4. Add offline tests for the spec and the argument conversion.

## New LLM provider

1. Add provider identity and defaults in `ira-llm` (`providers` module) and
   the client dispatch arm.
2. Implement a protocol adapter (payload builders + response parsers in
   `protocol`) for the provider's wire format, including tool-call
   translation if supported.
3. Add streaming (`chat_<name>_stream`) if the provider supports it.
4. Seed the catalog row (`providers.kind`) via migration or let the panel
   create it; empty keys fall back to an environment variable.
5. Add offline payload/response tests; provider failures must surface as
   `LlmError`, never panics.

## New speech engine

1. Implement the relevant synchronous trait — `SttEngine` (returns a
   transcript) or `TtsEngine` (returns mono PCM + sample rate) — in
   `ira-stt`/`ira-tts`. Async HTTP bridges go through
   `Handle::current().block_on` and may only be called from the engine's
   blocking thread.
2. Add the engine row (`role`, `kind`, `config`) and select it in the
   catalog (`SetEngine`).
3. Wire construction in `ira-daemon` (`build_stt` pattern: kind match +
   credential check + `Null*` fallback with a warning).
4. Keep playback in `ira-audio`; the session state machine is engine-agnostic.

## New catalog operation

1. Add the variant to `ira-store::DbOp` and persist it in `apply` (which
   reloads and returns the `Snapshot`).
2. Expose it through `ira-api` DTOs if the frontend reads the result.
3. Mirror the tag in `desktop/src/types.ts` — snake_case `op` discriminant
   must match exactly.
4. Wire the UI (`Catalog.tsx` drafts → `onOp`); the TUI settings screen only
   if the edit must be available there.
5. `ira-server` forwards `Op` generically; no per-operation change needed.

## New schema change

1. Add a numbered file under `deploy/postgres/migrations/`; keep it
   idempotent (`IF NOT EXISTS`, `ON CONFLICT DO NOTHING`) and merge the
   current state into `init.sql` for fresh databases.
2. Record the version in `schema_migrations` exactly as `store::migrate`
   applies it.
3. Never remove columns in the same release that stops writing them.

## New frontend doc article (this app)

1. Write `docs-app/content/en/<slug>.md` and `docs-app/content/es/<slug>.md`.
2. Add one entry to `docs-app/content/index.json` (`slug`, `section`,
   localized `title` + `summary`).
3. `npm run build` in `docs-app/` emits `desktop/public/docs/`; no code
   changes required.

## Verification commands

```sh
cargo fmt --all -- --check
cargo test -p ira-core -p ira-llm -p ira-tools -p ira-api -p ira-server
cargo doc --workspace --no-deps
cd desktop && npm run build
cd docs-app && npm run build
```
