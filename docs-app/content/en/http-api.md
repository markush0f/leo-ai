# HTTP API Reference

`ira-server` exposes the same catalog and chat engine as the Tauri shell over
HTTP. It binds `127.0.0.1:8787` by default; `--bind` or `IRA_HTTP_BIND`
override the address (e.g. `0.0.0.0:8787` for LAN use). Binding off loopback
still requires the token — every request must authenticate.

## Authentication

All `/api` routes pass through bearer authorization:

- Token source: `IRA_HTTP_TOKEN`, else the file `~/.ira/http.token`
  (generated on first server start).
- Clients send `Authorization: Bearer <token>`.
- The Vite dev proxy injects the token automatically for `/api` during
  development.
- The same token is used by the `ira-realtime` service to call the API.

## Response envelope

Success responses are JSON DTOs. Errors are
`{"error": "<message>"}` with a non-2xx status (`400` for empty messages,
`500` for internal failures). The frontend `http<T>()` helper turns any `error`
field into a thrown `Error`.

## Endpoints

Mounted under `/api`:

| Method | Path | Body | Returns | Notes |
| --- | --- | --- | --- | --- |
| `GET` | `/api/health` | — | `{ok, db}` | `db` is a live connectivity probe. |
| `GET` | `/api/snapshot` | — | `Snapshot` | Full catalog + settings DTO. |
| `POST` | `/api/apply` | `Op` | `Snapshot` | Applies one catalog edit. |
| `GET` | `/api/services` | — | `Services` | Host service board state (Compose). |
| `POST` | `/api/services` | — | `Services` | Starts services marked for boot. |
| `POST` | `/api/services/{id}` | `{action, port?, name?, description?}` | `Services` | `action` ∈ `start`, `stop`, `autostart`, `manual`, `port`, `meta`. |
| `GET` | `/api/chats` | — | `Conversation[]` | Recent conversations. |
| `POST` | `/api/chats` | — | `Conversation` | Creates one. |
| `GET` | `/api/chats/{id}` | — | `Turn[]` | Full transcript. |
| `POST` | `/api/chats/{id}/messages` | `{text}` | `{text}` | One blocking chat turn; empty text → `400`. |
| `POST` | `/api/chats/{id}/messages/stream` | `{text}` | NDJSON stream | Streaming turn, below. |
| `GET` | `/api/databases` | — | `DatabaseConnection[]` | Credential status only, never values. |
| `POST` | `/api/databases` | `DatabaseInput` | `DatabaseConnection` | Registers a connection (password encrypted). |
| `PUT` | `/api/databases/{id}` | `DatabaseInput` | `DatabaseConnection` | Updates it. |
| `DELETE` | `/api/databases/{id}` | — | `{ok}` | Removes it. |
| `POST` | `/api/databases/{id}/test` | — | `DatabaseTest` | Connects, records `last_test_ok`/error. |
| `GET` | `/api/databases/{id}/json` | — | JSON dump | Full export via `ira-pgjson` (schema + rows). |
| `GET` | `/api/databases/{id}/schema` | — | JSON dump | Schema-only export. |
| `POST` | `/api/codex/login` | `{provider_id}` | `CodexLogin` | Starts the Codex device login. |
| `POST` | `/api/codex/login/{id}/finish` | — | `Snapshot` | Completes it and reloads the catalog. |

## Streaming chat (NDJSON)

`POST /api/chats/{id}/messages/stream` returns
`Content-Type: application/x-ndjson` with `Cache-Control: no-cache` and
`x-accel-buffering: no`. Each line is one event:

```json
{"type":"delta","text":"partial"}
{"type":"reset"}
{"type":"done"}
{"type":"error","error":"message"}
```

- `delta` — append partial assistant text (may arrive repeatedly; the client
  merges).
- `reset` — discard the accumulated reply and restart (used when the tool
  loop retries without tools).
- `done` — turn complete; the reply and the user message are persisted.
- `error` — turn failed; the error is also stored as an `error` message.

Server-side, the turn runs on a spawned task feeding a bounded channel
(depth 32); a dropped body aborts the relay. If the client disconnects
mid-stream, generation continues to completion and is persisted.

## Static web root

When `desktop/dist` (auto-discovered next to the binary or via
`IRA_WEB_ROOT`) contains `index.html`, the same server also serves the built
frontend — including this documentation app at `/docs/` — so a production
deployment is one process.

## Concurrency

Chat turns are serialized per conversation by an in-process lock; two
surfaces chatting in the same conversation queue instead of interleaving.
