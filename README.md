# Leo

Local-first assistant for Linux with terminal, Telegram, and desktop chat interfaces,
plus a separate voice daemon. Chat supports Grok, GPT, Ollama, and Claude through a
shared provider catalog and tool registry.

## Start here

- [Architecture and crate map](crates.md): responsibilities, data flow, and extension points.
- [Voice configuration template](config/leo-ai.example.toml): devices, VAD, and voice providers.
- [Environment template](.env.example): credentials and service configuration.
- [Desktop design](DESIGN.md) and [product context](PRODUCT.md): interface conventions.

## Requirements

- A recent Rust toolchain with Rust 2024 support and Cargo.
- PostgreSQL 16; the included Compose service exposes it on port `5439`.
- For voice: PulseAudio or PipeWire's Pulse compatibility server and the
  `libpulse.so.0` / `libpulse-simple.so.0` runtime libraries. The audio build script
  adds `/usr/lib64` to the native library search path.
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
select a provider and model. PostgreSQL stores providers, models, the active
selection, and the system prompt; conversation history stays in memory.

Database URL precedence is `LEO_DATABASE_URL`, then `DATABASE_URL`, then
`postgres://leo:leo@127.0.0.1:5439/leo?sslmode=disable`.

## Run desktop chat

Run these commands from `desktop/`:

```sh
npm install
npm run tauri dev
```

For a browser-only preview, use `npm run dev`. The preview uses mock service
responses and an in-memory catalog rather than native database and voice access.
The development launcher defaults to port `5179` (`LEO_DEV_PORT` overrides it)
and uses `fuser -k` to stop any process already listening on that port.

## Run Telegram

Set `TELEGRAM_BOT_TOKEN` and `TELEGRAM_ALLOW_USERS` in the environment or `.env`,
then run:

```sh
cargo run -p leo-telegram
```

The allowlist is required: an empty list allows nobody. Plain text reaches the
model in private chats; groups accept commands only. `/help` lists commands.

## Run voice

Copy the [voice template](config/leo-ai.example.toml) to
`~/.config/leo-ai/config.toml` under the default Linux configuration directory.
Set `XAI_API_KEY` to enable Grok transcription, then run:

```sh
cargo run -p leo-daemon
```

From another terminal:

```sh
cargo run -p leo-ctl -- status
cargo run -p leo-ctl -- listen
cargo run -p leo-ctl -- stop
cargo run -p leo-ctl -- speak "Hello"
cargo run -p leo-ctl -- shutdown
```

Voice uses the model configured in TOML, independently of the chat catalog.
The template selects Ollama; the code default without an override is Grok.
Wake-word detection is currently a no-op, so use `listen` to activate it.
The current TTS fallback plays a beep, not spoken text. Without an STT key,
captured speech does not produce a transcript.

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
