# Deployment and Services

## Docker Compose stack

`docker-compose.yml` at the repository root manages the long-running
services; the desktop service board (`/api/services`) can start or stop each
one.

| Service | Image | Publish | Role |
| --- | --- | --- | --- |
| `postgres` | `postgres:16` | `127.0.0.1:5439` | Catalog database (`ira`/`ira`). |
| `toolbox` | official MCP Toolbox pinned to `1.11.0` | `127.0.0.1:5000` | Local database tool bridge; hot-reloads `.ira/toolbox`. Override image with `MCP_TOOLBOX_IMAGE`. |
| `ira-realtime` | built from `services/ira-realtime` | internal `8765` | Browser voice bridge (WebSocket, 16 kHz PCM). Talks to Ira over `IRA_REALTIME_IRA_URL` with the bearer token. |
| `ira-gateway` | `caddy:2-alpine` | `127.0.0.1:8790` | Front for realtime (`/realtime`) and projects services; `/healthz` healthcheck. |
| `projects-api` (+ projects-mcp) | `services/projects-api` | internal | Project/task API behind the gateway. |
| `colibri` | built from `deploy/colibri` | — | Optional local engine container (`COLIBRI_RAM`, default 16). |

```sh
docker compose up -d postgres toolbox    # minimum for chat
docker compose up -d                     # full board
```

## Launcher scripts

| Script | Behavior |
| --- | --- |
| `scripts/start-ira.sh` | Brings Postgres + Toolbox up, waits for health, installs frontend deps when missing, starts `ira-server` + Vite, opens services on `127.0.0.1`. Containers persist after the frontend closes. |
| `scripts/start-ira-realtime.sh` | Starts the realtime voice service standalone. |
| `scripts/run-toolbox.sh` | Runs the Toolbox container with the generated config. |

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `IRA_DATABASE_URL` / `DATABASE_URL` | `postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable` | Catalog connection (first wins). |
| `IRA_MASTER_KEY` | auto `.ira/master.key` | Encrypts provider keys and DB passwords. |
| `IRA_HTTP_TOKEN` | auto `~/.ira/http.token` | Bearer token guarding `/api`. |
| `IRA_HTTP_BIND` | `127.0.0.1:8787` | `ira-server` listen address; `0.0.0.0:8787` for LAN. |
| `IRA_WEB_ROOT` | discovered `desktop/dist` | Static frontend root served by `ira-server`. |
| `MCP_TOOLBOX_URL` | `http://127.0.0.1:5000` | Database tools bridge. |
| `MCP_TOOLBOX_IMAGE` | pinned official | Toolbox image override. |
| `XAI_API_KEY` | — | Grok key fallback (also STT voice). |
| `GITHUB_TOKEN` / `GH_TOKEN` | — | GitHub tools. |
| `GOOGLE_ACCESS_TOKEN` / `GOOGLE_API_KEY` | — | Google tools. |
| `TELEGRAM_BOT_TOKEN` / `TELEGRAM_ALLOW_USERS` | — | Telegram bot; empty allowlist = nobody. |
| `IRA_UID` / `IRA_GID` | `1000:1000` | Ownership of runtime files. |
| `IRA_REALTIME_MODE` | `ira` | `echo` for loopback testing (voice refuses to start in echo). |
| `IRA_REALTIME_TTS` / `_VOICE` / `_LANGUAGE` / `_QUANTIZE` | `pocket`/`lola`/`spanish`/`true` | Realtime speech synthesis. |
| `IRA_DEV_PORT` / `IRA_HTTP_PORT` | `5179` / `8787` | Dev launcher ports. |
| `RUST_LOG` | `info` | Tracing filter for all binaries. |

`.env` is loaded by every binary through `ira_llm::load_dotenv()` **without
overriding** exported variables. Copy from `.env.example`.

## Production topology

One command builds everything a single-box deployment needs:

```sh
cd desktop && npm run build           # dist/ + public/docs/ bundled
IRA_HTTP_BIND=127.0.0.1:8787 cargo run -p ira-server
```

`ira-server` then serves the app at `:8787`, the docs app at `:8787/docs/`,
and the API at `:8787/api` (bearer-guarded). Tauri (`npm run desktop`)
replaces the HTTP leg with native commands; same `App` underneath.

## Health checks

```sh
curl -H "Authorization: Bearer $(cat ~/.ira/http.token)" \
  http://127.0.0.1:8787/api/health          # {"ok":true,"db":true}
curl http://127.0.0.1:8790/healthz          # gateway
docker compose ps                            # container states
```

Store integration tests skip when Postgres or Ollama is unavailable — a green
test run alone does not prove the services work.

## Failure semantics to expect

- Missing database → every binary fails at startup with the compose hint.
- Missing provider key → daemon disables the LLM (`NullLlm`) and warns;
  chat surfaces show "falta api key"; TUI points to settings.
- Missing `TELEGRAM_BOT_TOKEN` → `ira-telegram` refuses to start.
- Occupied dev ports → launcher fails safely rather than rebinding.
- Toolbox down → database tools are simply unavailable to the model.
