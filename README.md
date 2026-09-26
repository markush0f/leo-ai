# Ira

Local-first assistant for Linux with terminal, Telegram, and desktop chat interfaces.
Chat supports Grok, GPT, Ollama, and Claude through a shared provider catalog and
tool registry. Voice is in the workspace but not part of the product yet.

## Start here

- [Architecture and crate map](crates.md): responsibilities, data flow, and extension points.
- [Environment template](.env.example): credentials and service configuration.
- [Desktop design](DESIGN.md) and [product context](PRODUCT.md): interface conventions.

## Requirements

- A recent Rust toolchain with Rust 2024 support and Cargo.
- PostgreSQL 16; the included Compose service exposes it on port `5439`.
- Database tools use a local [MCP Toolbox](https://github.com/googleapis/mcp-toolbox)
  container managed by Docker Compose.
- For desktop: Node.js compatible with Vite 8, npm, and Tauri 2's Linux native
  dependencies, including WebKitGTK 4.1 and GTK 3 development packages.
- A provider API key, or a running Ollama server with a downloaded model.

## Run terminal chat

From the repository root:

```sh
cp .env.example .env
docker compose up -d postgres toolbox
cargo run -p ira-tui --bin ira
```

Set credentials in `.env` or the provider catalog. Use the TUI's settings to
select a provider and model. PostgreSQL stores providers, models, engines,
settings, secrets, and conversation history.

Database URL precedence is `IRA_DATABASE_URL`, then `DATABASE_URL`, then
`postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable`.

Chat queries databases through a **local** MCP Toolbox container, not a hosted
server. Compose pulls the official image pinned to `1.11.0` and publishes
`127.0.0.1:5000`. Override it with `MCP_TOOLBOX_IMAGE` when needed.

Set `MCP_TOOLBOX_URL=http://127.0.0.1:5000`. If `IRA_MASTER_KEY` is unset,
Ira creates `.ira/master.key` on first use and reuses it. The web
panel writes encrypted PostgreSQL credentials to Ira's catalog and renders the
enabled connections into `.ira/toolbox` for Toolbox hot reload. Database users
must have read-only grants; the session default alone is not an authorization
boundary. Use `host.docker.internal` for PostgreSQL running on the Docker host.
Set `IRA_UID` and `IRA_GID` when Ira writes runtime files under a user other
than `1000:1000`.

`./scripts/start-ira.sh` also starts Veritas Kanban (`--profile kanban`).
The board is `http://127.0.0.1:3001`. Ira inserts tasks through the MCP gateway
at `VERITAS_MCP_URL=http://127.0.0.1:3100` with an `agent` key (`kanban_create_task`,
`kanban_update_task`, `kanban_invoke`). The desktop service board can start or
stop each Compose service. Local defaults stay on loopback; override
`VERITAS_ADMIN_KEY`, `VERITAS_AGENT_KEY`, and `VERITAS_JWT_SECRET` before
exposing the board.

## Run desktop chat

Para arrancar Postgres, MCP Toolbox, la API, WhatsApp y el frontend web con un solo comando:

```sh
./scripts/start-ira.sh
```

El script espera a que los contenedores estén sanos, instala las dependencias
del frontend cuando faltan y abre los servicios en `127.0.0.1`. Los contenedores
permanecen activos al cerrar el frontend.

Ira Realtime también arranca solo:

```sh
./scripts/start-ira-realtime.sh
```

Run these commands from `desktop/`:

```sh
npm install
npm run desktop
```

## Run in the browser

The browser cannot call Ollama directly (CORS). `ira-server` is the HTTP face
of the same catalog: it talks to PostgreSQL and to Ollama (or Grok, GPT, Claude)
on the machine where it runs.

From `desktop/`, with PostgreSQL up:

```sh
npm run web
```

That starts `ira-server` on `127.0.0.1:8787` and Vite on `5179` (proxying `/api`).
Open `http://127.0.0.1:5179`. The launcher fails safely when either port is
occupied (`IRA_DEV_PORT` / `IRA_HTTP_PORT` override them).

To listen on the LAN (phone, another computer):

```sh
npm run build
IRA_HTTP_BIND=0.0.0.0:8787 cargo run -p ira-server
```

Then open `http://<esta-máquina>:8787`. Binding off loopback lets anyone on that
network chat and run tools; keep it on a trusted LAN.

## Run Telegram

Set `TELEGRAM_BOT_TOKEN` and `TELEGRAM_ALLOW_USERS` in Postgres settings, the
environment, or `.env`, then run:

```sh
cargo run -p ira-telegram
```

The allowlist is required: an empty list allows nobody. Plain text reaches the
model in private chats; groups accept commands only. `/help` lists commands.

## Run WhatsApp

Requires `ira-server` on `127.0.0.1:8787`. Then:

```sh
cd services/ira-whatsapp
npm install
npm start
```

Scan the QR with the WhatsApp account that should answer as Leo, or open
WhatsApp in the desktop window: status, QR, allowlist, and changing the linked
number. By default Leo only replies in that account's self-chat.
`WHATSAPP_ALLOW_PHONES` adds other private senders; the window can change that
list without a restart. `npm run pair` deletes the session
(`WHATSAPP_AUTH_DIR`, default `~/.config/ira-ai/whatsapp`) and prints a new QR.
The control API listens on `127.0.0.1:8790`. This is not the official WhatsApp
API.

## Voice (later)

`ira-daemon` and `ira-ctl` remain in the workspace. They are not wired into
TUI, Telegram, or desktop. Do not run them as part of the current product.

## Development checks

```sh
cargo fmt --all -- --check
cargo doc --workspace --no-deps
cargo test -p ira-audio -p ira-vad -p ira-stt -p ira-core -p ira-llm -p ira-tools -p ira-tools-db -p ira-api -p ira-server
```

Run `npm run build` from `desktop/` to type-check and bundle the frontend.
Rustdoc output starts at `target/doc/`; each library documents its public entry
points and operational contracts. Workspace-wide builds also require desktop
native dependencies. Store integration tests may skip when PostgreSQL or Ollama
is unavailable, so a passing run alone does not confirm those services work.

## Documentation conventions

Write technical documentation in English. Use Rustdoc for module responsibilities
and public contracts, JSDoc for frontend boundaries, and inline comments for
non-obvious decisions. Document units, blocking behavior, failure semantics,
ownership, and limitations where they matter. Keep user-facing copy separate
from technical documentation and update contracts alongside code changes.
