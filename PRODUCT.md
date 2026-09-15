# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

React + TypeScript + Vite + Tauri 2 + Rust. Confirmed by the user. The UI is a web surface inside a Tauri desktop shell; the Rust side talks to `leo-store`, `leo-llm`, `leo-tools`, and `leo-ipc`. Runtime is a local desktop app, not a hosted site.

## Users

Markus, using Leo as a personal assistant on his Linux workstation. He already talks to Leo from the TUI (`leo`) and Telegram; he wants a window for the same job when he is not in a terminal.

## Product Purpose

Leo is a local personal assistant: chat with an LLM that can call tools, pick the model from a shared catalog, and (when the daemon is running) listen and speak. Success is being able to converse and change provider, model, API key, system prompt, and voice-session state without opening the TUI or `leo-ctl`.

## Positioning

One catalog (Postgres) and one tool registry feed every surface. The desktop app is another face of that catalog, not a second product with its own keys and history store.

## Operating Context

- The catalog lives in Postgres (`docker compose up -d`); chat history stays in memory.
- Voice is a separate process (`leo-daemon`) controlled over a Unix socket (`leo-ctl`: status, listen, stop, speak, shutdown).
- Tools register from the environment (files, shell, system, weather always; AppFlowy/GitHub/Google/Home Assistant when credentials exist).
- Keys may sit in the catalog or in env (`XAI_API_KEY`, etc.).
- Copy and UI are in Spanish, matching TUI and Telegram.

## Capabilities and Constraints

Confirmed for this surface:

- Minimalist chat against the active catalog model, through `leo-tools::chat` (tool loop included).
- Configure the catalog the TUI configures: providers (kind, API key, base URL, rename, create, delete), models (create, rename, delete, activate), system prompt, clear conversation.
- Control the voice daemon: status, listen, stop, speak, shutdown.
- Not in this surface: Telegram token/allowlist, editing `~/.config/leo-ai/config.toml`, wake-word training.

## Brand Commitments

- Name: Leo.
- Voice: Spanish, clear and direct.
- Existing surfaces: TUI (`leo`), Telegram (`leo-telegram`), voice daemon (`leo-daemon` + `leo-ctl`).

## Evidence on Hand

- Working TUI chat + settings (`crates/leo-tui`).
- Catalog schema in `deploy/postgres/init.sql`.
- No existing web UI, logo, or marketing assets. Do not invent customers, benchmarks, or hosted URLs.

## Product Principles

- One catalog, many faces.
- Chat is the job; settings are at hand, not a second app.
- Local-first: keys and models stay on this machine.
- Familiar controls for operate tasks; no invented chat chrome.
- Spanish UI, same words as the TUI where they already exist.

## Accessibility & Inclusion

No product-specific standard was set. Keyboard use and readable contrast are expected because the incumbent TUI is keyboard-first.
