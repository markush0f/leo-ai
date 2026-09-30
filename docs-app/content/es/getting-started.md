# Primeros pasos

Ira es un asistente local para Linux con interfaces de terminal, Telegram,
escritorio y navegador. Todo se ejecuta en tu máquina: un workspace de Cargo
con crates `ira-*`, un catálogo en PostgreSQL y un frontend React/Tauri.

## Requisitos

- Toolchain de Rust con soporte de Rust 2024 y Cargo.
- PostgreSQL 16 — el servicio de Compose incluido lo expone en el puerto `5439`.
- Node.js compatible con Vite 8 y npm; para escritorio, dependencias nativas
  de Tauri 2 en Linux (WebKitGTK 4.1, paquetes de desarrollo de GTK 3).
- Las herramientas de base de datos usan un contenedor local de
  [MCP Toolbox](https://github.com/googleapis/mcp-toolbox) gestionado por
  Docker Compose.
- Una API key de proveedor, o un servidor Ollama con un modelo descargado.

## Primera ejecución

```sh
cp .env.example .env
docker compose up -d postgres toolbox
cargo run -p ira-tui --bin ira        # chat de terminal
```

Define credenciales en `.env` o en el catálogo de proveedores. Usa los
ajustes del TUI para elegir proveedor y modelo. PostgreSQL guarda
proveedores, modelos, motores, ajustes, secretos e historial de conversaciones.

## Precedencia de la URL de base de datos

1. `IRA_DATABASE_URL`
2. `DATABASE_URL`
3. `postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable`

## Arranca todo con un comando

Desde la raíz del repositorio:

```sh
./scripts/start-ira.sh
```

El script espera a que los contenedores de Compose estén sanos, instala las
dependencias del frontend cuando faltan y abre los servicios en `127.0.0.1`.
Los contenedores permanecen activos al cerrar el frontend.

Ira Realtime arranca aparte:

```sh
./scripts/start-ira-realtime.sh
```

## Chat de escritorio

Desde `desktop/`:

```sh
npm install
npm run desktop
```

## En el navegador

El navegador no puede llamar a Ollama directamente (CORS). `ira-server` es la
cara HTTP del mismo catálogo: habla con PostgreSQL y con Ollama (o Grok, GPT,
Claude) en la máquina donde se ejecuta.

Desde `desktop/`, con PostgreSQL arriba:

```sh
npm run web
```

Eso arranca `ira-server` en `127.0.0.1:8787` y Vite en `5179` (proxy de
`/api`). Abre `http://127.0.0.1:5179`. El lanzador falla de forma segura si
algún puerto está ocupado; `IRA_DEV_PORT` / `IRA_HTTP_PORT` los cambian.

Para escuchar en la LAN (móvil, otro equipo):

```sh
npm run build
IRA_HTTP_BIND=0.0.0.0:8787 cargo run -p ira-server
```

Luego abre `http://<esta-máquina>:8787`. Sacarlo de loopback permite que
cualquiera de esa red chatee y ejecute herramientas; mantenlo en una LAN de
confianza.

## Telegram

Define `TELEGRAM_BOT_TOKEN` y `TELEGRAM_ALLOW_USERS` en los ajustes de
Postgres, el entorno o `.env`, y luego:

```sh
cargo run -p ira-telegram
```

La allowlist es obligatoria: una lista vacía no permite a nadie. En chats
privados el texto llega al modelo; en grupos solo se aceptan comandos.
`/help` lista los comandos.

## Voz (diferida)

`ira-daemon` e `ira-ctl` siguen en el workspace. No están conectados al TUI,
Telegram, escritorio ni navegador como producto actual; el camino de voz del
navegador pasa por el servicio `ira-realtime` (ver
[Despliegue y servicios](#/deployment)).

## App de documentación

Este lector es su propia app Vite en `docs-app/`. Los artículos son Markdown
bajo `docs-app/content/{en,es}/`, listados en `docs-app/content/index.json`.

```sh
cd docs-app
npm install
npm run dev      # http://127.0.0.1:5190
npm run build    # genera desktop/public/docs (servido por desktop + ira-server en /docs/)
```

Añade un artículo creando `content/en/<slug>.md` y `content/es/<slug>.md`
más una entrada en `index.json`. Sin cambios de código.

## Comprobaciones de desarrollo

```sh
cargo fmt --all -- --check
cargo doc --workspace --no-deps
cargo test -p ira-audio -p ira-vad -p ira-stt -p ira-core -p ira-llm -p ira-tools -p ira-tools-db -p ira-api -p ira-server
```

Ejecuta `npm run build` desde `desktop/` (y desde `docs-app/`) para
comprobar tipos y empaquetar los frontends. Los tests de integración del
store pueden saltarse si PostgreSQL u Ollama no están disponibles: que pasen
no confirma por sí solo que esos servicios funcionen.
