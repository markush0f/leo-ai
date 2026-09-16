# Leo

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
- For desktop: Node.js compatible with Vite 8, npm, and Tauri 2's Linux native
  dependencies, including WebKitGTK 4.1 and GTK 3 development packages.
- A provider API key, or a running Ollama server with a downloaded model.

## Run terminal chat

From the repository root:

```sh
cp .env.example .env
docker compose up -d postgres
cargo run -p leo-tui --bin leo
```

Set credentials in `.env` or the provider catalog. Use the TUI's settings to
select a provider and model. PostgreSQL stores providers, models, engines,
settings, secrets, and conversation history.

Database URL precedence is `LEO_DATABASE_URL`, then `DATABASE_URL`, then
`postgres://leo:leo@127.0.0.1:5439/leo?sslmode=disable`.

## Run desktop chat

Run these commands from `desktop/`:

```sh
npm install
npm run desktop
```

For a browser-only preview, use `npm run web`. The preview uses mock service
responses and an in-memory catalog rather than native database access.
The development launcher defaults to port `5179` (`LEO_DEV_PORT` overrides it)
and uses `fuser -k` to stop any process already listening on that port.

## Run Telegram

Set `TELEGRAM_BOT_TOKEN` and `TELEGRAM_ALLOW_USERS` in Postgres settings, the
environment, or `.env`, then run:

```sh
cargo run -p leo-telegram
```

The allowlist is required: an empty list allows nobody. Plain text reaches the
model in private chats; groups accept commands only. `/help` lists commands.

## Voice (later)

`leo-daemon` and `leo-ctl` remain in the workspace. They are not wired into
TUI, Telegram, or desktop. Do not run them as part of the current product.

## Development checks

```sh
cargo fmt --all -- --check
cargo doc --workspace --no-deps
cargo test -p leo-audio -p leo-vad -p leo-stt -p leo-core -p leo-llm -p leo-tools
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
