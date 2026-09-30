# Shared DTO and Op Contract

Three surfaces (Tauri desktop, browser via `ira-server`, TUI) read and edit
the same catalog. `ira-api` is the single Rust definition point; the frontend
mirrors it in TypeScript. Keeping the two in sync is a manual contract.

## Layers

```text
desktop/src/types.ts   ← mirror —   crates/ira-api/src/dto.rs
        │                                   │
   desktop/src/api.ts                   ira_api::App
   (invoke | fetch)                    (snapshot, apply, chat, …)
        │                        ┌──────────┴──────────┐
        └── Tauri commands ──────┤ ira-server handlers │
                                 └─────────────────────┘
```

- `ira-api::App` owns the catalog operations used by both transports:
  `snapshot()`, `apply(Op)`, `chat()`, `chat_stream()`, `list_chats()`,
  services, databases, Codex login.
- Tauri commands in `desktop/src-tauri/src/lib.rs` are thin wrappers over the
  same `App`.
- `ira-server` handlers are thin wrappers over the same `App` plus the bearer
  guard.

## Snapshot DTO

Serialized catalog copy exposed to the frontend: providers, models, engines,
settings, and credential **status** flags (e.g. "has key") — never key
values. Internal rows never cross the boundary; DTO conversion happens in
`ira-api/src/dto.rs`.

## Op — tagged catalog edits

`ira-store::DbOp` serializes with a snake_case `op` tag; the frontend `Op`
union in `desktop/src/types.ts` lists the same variants:
`activate_provider`, `activate_model`, `set_kind`, `set_system`,
`set_voice_system`, `set_api_key`, `set_base_url`, `new_provider`,
`new_model`, `rename_provider`, `rename_model`, `delete_provider`,
`delete_model`, `set_engine`, `set_stt_language`, `set_thinking`,
`set_model_effort`, `set_tools_enabled`, `set_tools_mutate`.

```ts
const op: Op = { op: "set_thinking", value: true };
const snap = await applyOp(op); // Tauri invoke("apply") or POST /api/apply
```

Both transports return the **full updated Snapshot**, so the frontend never
patches state optimistically.

## Stream events

`ChatStreamEvent` (`ira-api`) is the NDJSON line shape and the Tauri
`Channel<ChatStreamEvent>` payload: `{type:"delta"|"reset"|"done"|"error"}`.
See [HTTP API Reference](#/http-api).

## Sync rules when extending

1. Add the variant in `ira-store/src/lib.rs` (`DbOp`) and persist it in
   `apply`.
2. Expose it through `ira-api` (DTO/`Op` if the frontend uses it).
3. Add the matching member to the `Op` union in `desktop/src/types.ts`.
4. Wire the UI in `Catalog.tsx` (drafts are local; operations go through
   `onOp`).
5. `ira-server` needs no change (it forwards `Op` generically); the TUI
   settings screen needs it only if the edit must be available there.

Field names, tag spellings, and optionality must match exactly — the tag is
part of the JSON shape, and mismatches fail deserialization at runtime, not
compile time.
