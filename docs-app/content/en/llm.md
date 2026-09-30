# ira-llm: Provider Transport

`ira-llm` is the provider-independent HTTP transport for chat models. It owns
no conversation state and never executes tools — it transports tool calls.

## Core types

| Type | Contract |
| --- | --- |
| `Client` | Provider settings + one reusable `reqwest` client. Built from the active catalog row via `Snapshot::client()`. |
| `ChatRequest` | System text, message history, tool schemas, reasoning options. `with_history(system, history)` is the standard constructor. |
| `ChatResponse` | Final assistant text plus any tool calls, model usage info. |
| `LlmError` | Transport/protocol failures, incl. `MissingKey(var)` (surfaced to users as "falta api key") and `ToolLoop` from the tool layer. |
| `protocol` | Payload builders and parsers kept separate so tests run offline. |

## Adapters

| Adapter | Providers | Wire protocol |
| --- | --- | --- |
| `openai_compat` | Grok, GPT | OpenAI-compatible `/chat/completions` messages and tool calls. |
| `ollama` | Ollama | Native `/api/chat` plus model discovery (`/api/tags`). |
| `claude` | Claude | Anthropic `/v1/messages` with tool-use translation. |
| `codex` | Codex (ChatGPT backend) | OAuth-style device login flow driven by `begin_codex_login` / `finish_codex_login` in `ira-api`. |

Provider identity and defaults live in `ira-llm/src/providers/`. Adding a
provider means extending that module, the client dispatch, and a protocol
adapter with offline payload/response tests.

## Streaming

`Client::chat_stream` exposes per-provider streaming (`chat_openai_stream`,
`chat_ollama_stream`, `chat_claude_stream`). Deltas are pushed through a
callback sink; `ira-api` relays them to the browser as NDJSON (see
[HTTP API Reference](#/http-api)).

## Environment

- `load_dotenv()` reads `.env` at process start **without overwriting**
  already-exported variables. Every binary (`ira`, `ira-daemon`,
  `ira-telegram`, `ira-server`) calls it first.
- Empty API keys fall back to the provider's environment variable.

## Blocking bridge (voice)

The voice daemon wraps the async client in `BlockingLlm`, which bridges to
`tokio::runtime::Handle::block_on`. `GrokStt` does the same. Call rules:

- Invoke only from a dedicated blocking thread while the runtime stays active.
- Never call `block_on` from inside an async task — it panics.
- Voice LLM requests contain the current user input plus the voice system
  prompt, not persistent chat history.
