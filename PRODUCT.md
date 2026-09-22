# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

React + TypeScript + Vite + Tauri 2 + Rust. Confirmed by the user. The UI is a
web surface: inside a Tauri desktop shell, or in any browser against
`ira-server`. The Rust side talks to `ira-store`, `ira-llm`, and `ira-tools`.
Runtime is this machine (desktop window or local HTTP), not a hosted site.

## Users

Markus, using Ira as a personal assistant on his Linux workstation. He already talks to Ira from the TUI (`ira`) and Telegram; he wants a window for the same job when he is not in a terminal.

## Product Purpose

Ira is a local personal assistant: chat with an LLM that can call tools and pick the model from a shared catalog. Success is being able to converse and change provider, model, API key, and system prompt without opening the TUI. Voice (listen/speak via `ira-daemon`) is deferred until chat is solid.

## Positioning

One catalog and conversation store (Postgres) and one tool registry feed every surface. The desktop app is another face of that catalog, not a second product with its own keys and history store.

## Operating Context

- The catalog, engines, settings, and chat history live in Postgres (`docker compose up -d`).
- Voice crates (`ira-daemon`, `ira-ctl`) exist in the workspace but are out of scope until chat ships.
- Tools register from the environment (files, shell, system, weather always; AppFlowy/GitHub/Google/Home Assistant when credentials exist; local MCP Toolbox when `MCP_TOOLBOX_URL` is set).
- Keys may sit in the catalog or in env (`XAI_API_KEY`, etc.).
- Copy and UI are in Spanish, matching TUI and Telegram.

## Capabilities and Constraints

Confirmed for this surface:

- Minimalist chat against the active catalog model, through `ira-tools::chat` (tool loop included).
- Start local Docker services Ira uses (Postgres and MCP Toolbox) from the window.
- Configure the catalog the TUI configures: providers (kind, API key, base URL, rename, create, delete), models (create, rename, delete, activate), system prompt, clear conversation.
- Not in this surface: voice daemon control, Telegram token/allowlist, editing `~/.config/ira-ai/config.toml`.

## Brand Commitments

- Name: Ira.
- Voice: Spanish, clear and direct.
- Existing surfaces: TUI (`ira`), Telegram (`ira-telegram`), desktop, browser via `ira-server`. Voice daemon later.

## Evidence on Hand

- Working TUI chat + settings (`crates/ira-tui`).
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
