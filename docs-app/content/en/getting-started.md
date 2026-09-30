# Getting Started

Ira is a local-first assistant for Linux with terminal, Telegram, desktop chat,
and browser interfaces. Everything runs on your machine: a Cargo workspace of
`ira-*` crates, a PostgreSQL catalog, and a React/Tauri desktop frontend.

## Requirements

- Rust toolchain with Rust 2024 support and Cargo.
- PostgreSQL 16 — the included Compose service exposes it on port `5439`.
- Node.js compatible with Vite 8 and npm; for desktop, Tauri 2 Linux native
  dependencies (WebKitGTK 4.1, GTK 3 development packages).
- Database tools use a local [MCP Toolbox](https://github.com/googleapis/mcp-toolbox)
  container managed by Docker Compose.
- A provider API key, or a running Ollama server with a downloaded model.

## First run

```sh
cp .env.example .env
docker compose up -d postgres toolbox
cargo run -p ira-tui --bin ira        # terminal chat
```

Set credentials in `.env` or through the provider catalog. Use the TUI's
settings to select a provider and model. PostgreSQL stores providers, models,
engines, settings, secrets, and conversation history.

## Database URL precedence

1. `IRA_DATABASE_URL`
2. `DATABASE_URL`
3. `postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable`

## Run everything with one command

From the repository root:

```sh
./scripts/start-ira.sh
```

The script waits for the Compose containers to become healthy, installs
frontend dependencies when missing, and opens the services on `127.0.0.1`.
Containers stay running after you close the frontend.

Ira Realtime starts separately:

```sh
./scripts/start-ira-realtime.sh
```

## Run desktop chat

From `desktop/`:

```sh
npm install
npm run desktop
```

## Run in the browser

The browser cannot call Ollama directly (CORS). `ira-server` is the HTTP face
of the same catalog: it talks to PostgreSQL and to Ollama (or Grok, GPT,
Claude) on the machine where it runs.

From `desktop/`, with PostgreSQL up:

```sh
npm run web
```

That starts `ira-server` on `127.0.0.1:8787` and Vite on `5179` (proxying
`/api`). Open `http://127.0.0.1:5179`. The launcher fails safely when either
port is occupied; `IRA_DEV_PORT` / `IRA_HTTP_PORT` override them.

To listen on the LAN (phone, another computer):

```sh
npm run build
IRA_HTTP_BIND=0.0.0.0:8787 cargo run -p ira-server
```

Then open `http://<this-machine>:8787`. Binding off loopback lets anyone on
that network chat and run tools; keep it on a trusted LAN.

## Run Telegram

Set `TELEGRAM_BOT_TOKEN` and `TELEGRAM_ALLOW_USERS` in Postgres settings, the
environment, or `.env`, then:

```sh
cargo run -p ira-telegram
```

The allowlist is required: an empty list allows nobody. Plain text reaches the
model in private chats; groups accept commands only. `/help` lists commands.

## Voice (deferred)

`ira-daemon` and `ira-ctl` remain in the workspace. They are not wired into
TUI, Telegram, desktop, or browser as part of the current product; the browser
voice path goes through the `ira-realtime` service instead (see
[Deployment and Services](#/deployment)).

## Documentation app

This reader is its own Vite app in `docs-app/`. Articles are Markdown under
`docs-app/content/{en,es}/` listed by `docs-app/content/index.json`.

```sh
cd docs-app
npm install
npm run dev      # http://127.0.0.1:5190
npm run build    # emits desktop/public/docs (served by desktop + ira-server at /docs/)
```

Add a new article by dropping `content/en/<slug>.md` and `content/es/<slug>.md`
plus one entry in `index.json`. No code changes needed.

## Development checks

```sh
cargo fmt --all -- --check
cargo doc --workspace --no-deps
cargo test -p ira-audio -p ira-vad -p ira-stt -p ira-core -p ira-llm -p ira-tools -p ira-tools-db -p ira-api -p ira-server
```

Run `npm run build` from `desktop/` (and from `docs-app/`) to type-check and
bundle the frontends. Store integration tests may skip when PostgreSQL or
Ollama is unavailable, so a passing run alone does not confirm those services
work.
