# Leo desktop

React and TypeScript frontend hosted by Tauri 2. The native backend shares the
PostgreSQL catalog, LLM client, and tool registry with terminal and Telegram chat.
Voice is deferred; this surface is chat and catalog only.

## Run

Start PostgreSQL from the repository root with `docker compose up -d postgres`.
Then, from this directory:

```sh
npm install
npm run desktop
```

Native development requires Rust and Tauri's Linux dependencies, including
WebKitGTK 4.1 and GTK 3 development packages. See the [root guide](../README.md)
for credentials and service configuration.

## Browser

```sh
npm run web
```

Starts `leo-server` (PostgreSQL catalog + Ollama and other providers) and Vite.
The UI calls `/api`; Vite proxies that to `127.0.0.1:8787`. This is live chat,
not a mock. The launcher uses port `5179` by default (`LEO_DEV_PORT` overrides
it) and `8787` for the API (`LEO_HTTP_PORT` / `LEO_HTTP_BIND`). It runs
`fuser -k` against those ports before starting.

Point another device at this machine with `LEO_HTTP_BIND=0.0.0.0:8787` after
`npm run build` so `leo-server` also serves `desktop/dist`.

## Code map

| File | Responsibility |
| --- | --- |
| `src/App.tsx` | Chat history, display bubbles, catalog state, and theme. |
| `src/Catalog.tsx` | Provider/model forms and catalog mutations. |
| `src/api.ts` | Tauri commands or `leo-server` HTTP (`/api`). |
| `src/types.ts` | DTOs and operation tags mirrored by Rust. |
| `src/theme.ts`, `src/styles.css` | Theme persistence and presentation. |
| `src-tauri/src/lib.rs` | Command handlers and shared service initialization. |
| `src-tauri/src/main.rs` | Native process entry point and renderer environment setup. |

Changing a bridge contract requires updating TypeScript types, `leo-api` DTOs,
and `leo-server` routes together. Catalog responses expose key availability, not
stored key values. Chat history is held in frontend memory and is separate from
persistent settings.

## Checks

```sh
npm run build
```

This type-checks TypeScript and builds the Vite bundle. Run Cargo checks from
the workspace root for the native backend.

## Linux rendering

The native entry point configures WebKit/GTK defaults for desktop rendering.
Check `src-tauri/src/main.rs` before overriding these settings when diagnosing
NVIDIA or Wayland-specific issues.
