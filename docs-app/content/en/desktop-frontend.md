# Desktop and Web Frontend

`desktop/` is a React 19 + Vite app that runs in two hosts: inside Tauri 2
(native commands) and in the browser (proxying `/api` to `ira-server`). The
same source powers both.

## Layout

| File | Responsibility |
| --- | --- |
| `src/main.tsx` | Mounts `App`, loads fonts and styles. |
| `src/App.tsx` | Shell: conversation state, bubbles, streaming, theme, sheet orchestration, service start, voice stage. |
| `src/Catalog.tsx` | Catalog editor sheet: local form drafts; every commit emits an `Op` through `onOp`. |
| `src/Databases.tsx` | Database connections sheet (CRUD, test, toolbox enablement). |
| `src/Services.tsx` | Host services board sheet: start/stop/autostart/port/meta per Compose service. |
| `src/VoiceStage.tsx` | Voice overlay driven by `voice.ts` callbacks. |
| `src/components/` | `Composer`, `Message` (renders `Markdown`), `Field`, focus hooks. |
| `src/Markdown.tsx` | GFM rendering via `react-markdown`, compiled to React elements (no `dangerouslySetInnerHTML`). |
| `src/api.ts` | Transport boundary (below). |
| `src/types.ts` | DTOs + `Op` union mirrored from `ira-api` (see [dto-contract](#/dto-contract)). |
| `src/theme.ts` | Theme preference persisted under `ira-theme`, applied via `data-theme` on `<html>`. |
| `src-tauri/` | Rust shell: thin `ira_api::App` command wrappers. |

## Transport split (api.ts)

`inTauri` (`"__TAURI_INTERNALS__" in window`) selects the path per function:

- **Tauri**: `invoke("snapshot")`, `invoke("apply", { op })`, streaming via
  `Channel<ChatStreamEvent>`.
- **Browser**: `http<T>()` against `/api` with `Authorization: Bearer` when
  `VITE_IRA_HTTP_TOKEN` is set. Connection failures throw a message telling
  the user to start `ira-server`.

Dev flow (`npm run web` → `scripts/dev.mjs`): starts `ira-server` on
`127.0.0.1:8787` and Vite on `5179`; the Vite proxy injects the bearer token
from `IRA_HTTP_TOKEN` or `~/.ira/http.token`. Ports fail safely when occupied
(`IRA_DEV_PORT` / `IRA_HTTP_PORT` override).

## Chat behavior

- `streamChat` consumes NDJSON lines, merging `delta`s into one assistant
  bubble (`mergeReply` handles out-of-band repeats); `reset` clears the
  partial; errors replace the visible bubble with an error bubble.
- The shell keeps a run counter (`chatRunRef`) so a cancelled/late stream
  cannot mutate newer state; rendering is coalesced with
  `requestAnimationFrame`.
- On boot, empty account → auto-create chat; `?catalog` deep-link opens the
  catalog sheet; `?demo` enables demo mode; `?theme=light|dark` overrides.
- Services poll every 4 s through `loadServices`.

## Voice path

`src/voice.ts` talks to the **ira-realtime** service, not the deferred
`ira-daemon`:

- Base URL: `VITE_IRA_REALTIME`, default `http://127.0.0.1:8790/realtime`
  (Caddy gateway → `ira-realtime` container).
- A WebSocket carries mic PCM upstream and playback + events
  (`transcript`, `reply`, `level`, `state`, `error`) downstream; the stage
  shows user transcript, Ira reply, and phase.
- Health probe on `/`; when the service runs in echo mode
  (`IRA_REALTIME_MODE` ≠ `ira`) voice is refused with a start hint.
- Transcripts and replies persist as ordinary conversation turns through the
  realtime service calling `ira-server`'s chat HTTP API.

## Accessibility conventions

- Sheet focus management via `useSheetFocus` (trap + restore), rail drawer
  marked `role="dialog"` on mobile.
- Background surfaces get `inert` while a sheet/rail is open.
- Streaming replies announce completion through an aria-live region.
- Reduced-motion preference respected (`useReducedMotion`).

## Documentation app access

The standalone docs app (`docs-app/`) builds into `desktop/public/docs/` and
is served statically by Vite/Tauri and by `ira-server` (web root). The rail's
**Documentación** item opens `/docs/` externally
(`openExternal` → Tauri opener or new tab).
