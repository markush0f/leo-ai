# Leo desktop

React and TypeScript frontend hosted by Tauri 2. The native backend shares the
PostgreSQL catalog, LLM client, and tool registry with terminal and Telegram chat.
Voice controls communicate with the separate daemon over Unix IPC.

## Run

Start PostgreSQL from the repository root with `docker compose up -d postgres`.
Then, from this directory:

```sh
npm install
npm run tauri dev
```

Native development requires Rust and Tauri's Linux dependencies, including
WebKitGTK 4.1 and GTK 3 development packages. See the [root guide](../README.md)
for credentials and service configuration.

## Browser preview

```sh
npm run dev
```

Preview uses in-memory mock responses, not live LLM, database, or voice services.
The launcher uses port `5179` by default (`LEO_DEV_PORT` overrides it) and runs
`fuser -k` against that port before starting Vite.

## Code map

| File | Responsibility |
| --- | --- |
| `src/App.tsx` | Chat history, display bubbles, catalog state, and voice polling. |
| `src/Catalog.tsx` | Provider/model forms and catalog mutations. |
| `src/api.ts` | Native commands and browser-preview implementations. |
| `src/types.ts` | DTOs and operation tags mirrored by Rust. |
| `src/theme.ts`, `src/styles.css` | Theme persistence and presentation. |
| `src-tauri/src/lib.rs` | Command handlers and shared service initialization. |
| `src-tauri/src/main.rs` | Native process entry point and renderer environment setup. |

Changing a bridge contract requires updating both TypeScript types and Rust
DTOs. Native catalog responses expose key availability, not stored key values.
Chat history is held in frontend memory and is separate from persistent settings.

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
