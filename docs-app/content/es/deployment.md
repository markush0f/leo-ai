# Despliegue y servicios

## Stack de Docker Compose

`docker-compose.yml` en la raíz gestiona los servicios de larga ejecución; el
tablero de servicios del escritorio (`/api/services`) puede arrancar o parar
cada uno.

| Servicio | Imagen | Publica | Rol |
| --- | --- | --- | --- |
| `postgres` | `postgres:16` | `127.0.0.1:5439` | Base de datos del catálogo (`ira`/`ira`). |
| `toolbox` | MCP Toolbox oficial fijado a `1.11.0` | `127.0.0.1:5000` | Puente local de herramientas de base de datos; hot-reload de `.ira/toolbox`. La imagen se cambia con `MCP_TOOLBOX_IMAGE`. |
| `ira-realtime` | se construye desde `services/ira-realtime` | interno `8765` | Puente de voz realtime del navegador (WebSocket, PCM 16 kHz). Llama a Ira por `IRA_REALTIME_IRA_URL` con el token bearer. |
| `ira-gateway` | `caddy:2-alpine` | `127.0.0.1:8790` | Front de realtime (`/realtime`) y servicios de proyectos; healthcheck en `/healthz`. |
| `projects-api` (+ projects-mcp) | `services/projects-api` | interno | API de proyectos/tareas tras el gateway. |
| `colibri` | se construye desde `deploy/colibri` | — | Contenedor opcional de motor local (`COLIBRI_RAM`, default 16). |

```sh
docker compose up -d postgres toolbox    # mínimo para chatear
docker compose up -d                     # tablero completo
```

## Scripts de arranque

| Script | Comportamiento |
| --- | --- |
| `scripts/start-ira.sh` | Levanta Postgres + Toolbox, espera a que estén sanos, instala dependencias del frontend si faltan, arranca `ira-server` + Vite y abre los servicios en `127.0.0.1`. Los contenedores siguen activos al cerrar el frontend. |
| `scripts/start-ira-realtime.sh` | Arranca el servicio de voz realtime solo. |
| `scripts/run-toolbox.sh` | Ejecuta el contenedor Toolbox con la config generada. |

## Variables de entorno

| Variable | Default | Significado |
| --- | --- | --- |
| `IRA_DATABASE_URL` / `DATABASE_URL` | `postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable` | Conexión del catálogo (gana la primera). |
| `IRA_MASTER_KEY` | auto `.ira/master.key` | Cifra claves de proveedor y contraseñas de BD. |
| `IRA_HTTP_TOKEN` | auto `~/.ira/http.token` | Token bearer que protege `/api`. |
| `IRA_HTTP_BIND` | `127.0.0.1:8787` | Dirección de escucha de `ira-server`; `0.0.0.0:8787` para LAN. |
| `IRA_WEB_ROOT` | `desktop/dist` detectado | Raíz estática del frontend servida por `ira-server`. |
| `MCP_TOOLBOX_URL` | `http://127.0.0.1:5000` | Puente de herramientas de base de datos. |
| `MCP_TOOLBOX_IMAGE` | oficial fijada | Override de la imagen Toolbox. |
| `XAI_API_KEY` | — | Fallback de la key de Grok (también STT de voz). |
| `GITHUB_TOKEN` / `GH_TOKEN` | — | Herramientas de GitHub. |
| `GOOGLE_ACCESS_TOKEN` / `GOOGLE_API_KEY` | — | Herramientas de Google. |
| `TELEGRAM_BOT_TOKEN` / `TELEGRAM_ALLOW_USERS` | — | Bot de Telegram; allowlist vacía = nadie. |
| `IRA_UID` / `IRA_GID` | `1000:1000` | Propiedad de los archivos de runtime. |
| `IRA_REALTIME_MODE` | `ira` | `echo` para pruebas de loopback (la voz se niega en modo eco). |
| `IRA_REALTIME_TTS` / `_VOICE` / `_LANGUAGE` / `_QUANTIZE` | `pocket`/`lola`/`spanish`/`true` | Síntesis de voz realtime. |
| `IRA_DEV_PORT` / `IRA_HTTP_PORT` | `5179` / `8787` | Puertos del lanzador de desarrollo. |
| `RUST_LOG` | `info` | Filtro de tracing para todos los binarios. |

`.env` lo cargan todos los binarios con `ira_llm::load_dotenv()` **sin
sobreescribir** variables ya exportadas. Copia desde `.env.example`.

## Topología de producción

Un comando construye todo lo que un despliegue de una máquina necesita:

```sh
cd desktop && npm run build           # dist/ + public/docs/ empaquetados
IRA_HTTP_BIND=127.0.0.1:8787 cargo run -p ira-server
```

`ira-server` sirve entonces la app en `:8787`, la documentación en
`:8787/docs/` y la API en `:8787/api` (con bearer). Tauri
(`npm run desktop`) sustituye la pata HTTP por comandos nativos; el mismo
`App` debajo.

## Comprobaciones de salud

```sh
curl -H "Authorization: Bearer $(cat ~/.ira/http.token)" \
  http://127.0.0.1:8787/api/health          # {"ok":true,"db":true}
curl http://127.0.0.1:8790/healthz          # gateway
docker compose ps                            # estado de contenedores
```

Los tests de integración del store se saltan si PostgreSQL u Ollama no están
disponibles: una corrida verde sola no prueba que los servicios funcionen.

## Semántica de fallos esperable

- Falta la base de datos → todos los binarios fallan al arrancar con la pista
  de compose.
- Falta la key del proveedor → el daemon desactiva el LLM (`NullLlm`) y
  avisa; las superficies de chat muestran "falta api key"; el TUI lleva a
  settings.
- Falta `TELEGRAM_BOT_TOKEN` → `ira-telegram` se niega a arrancar.
- Puertos de desarrollo ocupados → el lanzador falla de forma segura en lugar
  de reasignar.
- Toolbox caído → las herramientas de base de datos simplemente no están
  disponibles para el modelo.
