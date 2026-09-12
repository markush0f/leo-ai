# Crates

Leo es un workspace Cargo (`crates/*`). Hay dos caminos que casi no se mezclan:

1. **Voz** — micrófono → transcripción → LLM → altavoz. Lo orquesta `leo-daemon`; lo controlas con `leo-ctl`.
2. **Chat** — texto en la TUI (`leo`) o en Telegram. Comparten Postgres (`leo-store`), el cliente LLM (`leo-llm`) y las tools (`leo-tools`).

El daemon de voz **no** usa `leo-store` ni `leo-tools`: lee `~/.config/leo-ai/config.toml` y habla con un único modelo.

```
                    ┌─────────────┐
                    │  leo-llm    │◄──────────────┐
                    └──────┬──────┘               │
                           │                      │
         voz               │              chat    │
                           │                      │
  leo-audio  leo-vad       │         leo-store    │
  leo-wake   leo-stt       │         leo-tools    │
  leo-tts                  │                      │
         \                 │                 /    │
          \                │                /     │
           ▼               ▼               ▼      │
        leo-core      leo-daemon        leo-tui ──┘  (bin: leo)
           ▲               ▲            leo-telegram (bin: leo-telegram)
           │               │
        leo-ipc ◄──────────┴── leo-ctl
```

## Binarios

| Crate | Comando | Para qué |
|---|---|---|
| `leo-tui` | `leo` | Chat en terminal |
| `leo-telegram` | `leo-telegram` | Chat por Telegram |
| `leo-desktop` | `npm run tauri dev` (en `desktop/`) | Chat y catálogo en una ventana (Tauri) |
| `leo-daemon` | `leo-daemon` | Asistente de voz en segundo plano |
| `leo-ctl` | `leo-ctl` | Mandar órdenes al daemon (`listen`, `stop`, `speak`, …) |

El resto son librerías.

---

## Chat

### `leo-tui` → `leo`

TUI con ratatui. Burbujas de chat, `/model` `/providers`, pantalla de ajustes (Tab).

Lee el catálogo de Postgres, construye el cliente LLM y pasa cada mensaje por `leo-tools::chat`. El historial de la sesión vive en memoria; proveedor, modelo y system prompt viven en `leo-store`.

### `leo-telegram` → `leo-telegram`

Bot por long-poll. Mismo catálogo y mismas tools que `leo`.

- Allowlist: `TELEGRAM_ALLOW_USERS` (si está vacía, no habla con nadie).
- En privado, el texto suelto va al LLM. En grupos ignora el texto y solo atiende comandos.
- Comandos: `/help`, `/status`, `/model`, `/providers`, `/clear`, `/system`.
- Mientras el modelo piensa, renueva `sendChatAction(typing)` para que Telegram muestre “escribiendo…”.

La librería (`src/lib.rs`) parsea comandos y el historial; el binario (`main.rs` + `tg.rs`) habla con la API de Telegram.

### `leo-desktop` → ventana

App Tauri 2 (`desktop/`): React + Vite. El mismo catálogo y el mismo `leo-tools::chat` que la TUI. Desde el borde (Tab) se editan proveedores, modelos, keys y el system prompt. Controles de voz: `escuchar`, `parar`, `decir`, `apagar` vía `leo-ipc`.

### `leo-store`

Catálogo en Postgres: proveedores, modelos, modelo activo y system prompt.

- URL: `LEO_DATABASE_URL` o `DATABASE_URL` (por defecto `postgres://leo:leo@127.0.0.1:5439/leo`).
- `Snapshot` es la foto que usan TUI y Telegram. `Snapshot::client()` arma el `leo-llm::Client` del modelo activo.
- `DbOp` son los cambios (activar modelo, guardar API key, etc.).
- Si el proveedor activo es Ollama, sincroniza `/api/tags` con la tabla `models`.

El esquema está en `deploy/postgres/init.sql`. Arranca la bbdd con `docker compose up -d`.

### `leo-llm`

Cliente HTTP unificado: **Grok**, **GPT**, **Ollama**, **Claude**.

- `Client::chat` manda un `ChatRequest` y devuelve `ChatResponse` (texto y, si aplica, `tool_calls`).
- Grok y GPT usan el protocolo OpenAI; Claude el suyo; Ollama `/api/chat`.
- También carga el `.env` (`load_dotenv`) y lista modelos de Ollama.

No ejecuta tools: solo las serializa y parsea. El bucle está en `leo-tools`.

### `leo-tools`

Núcleo: trait `Tool`, `Registry`, `Context` y el bucle (el modelo pide una función → se ejecuta → se le devuelve el resultado, hasta 8 vueltas). TUI y Telegram usan este crate; el daemon de voz no.

Las implementaciones viven en `crates/tools/*`. `Registry::from_env()` registra las locales siempre y las de APIs solo si hay credenciales:

| Crate | Tools | Cuándo |
|---|---|---|
| `leo-tools-files` | `read_file`, `write_file`, `list_directory`, `search_files`, `move_file`, `copy_file`, `remove_file` | siempre |
| `leo-tools-shell` | `execute_command`, `execute_script` | siempre |
| `leo-tools-system` | procesos y escritorio (`list_processes`, `open_url`, portapapeles, …) | siempre |
| `leo-tools-weather` | `get_weather`, `get_forecast` | siempre (Open-Meteo) |
| `leo-tools-appflowy` | páginas (`appflowy_create_page`, `appflowy_write`, …) | `APPFLOWY_BASE_URL` + credenciales |
| `leo-tools-github` | issues y pull requests | `GITHUB_TOKEN` o `GH_TOKEN` |
| `leo-tools-google` | Google Calendar | `GOOGLE_ACCESS_TOKEN` o `GOOGLE_API_KEY` |
| `leo-tools-home-assistant` | estados y servicios | `HOME_ASSISTANT_URL`/`HASS_URL` + token |
| `leo-tools-notion` | — | pendiente |
| `leo-tools-spotify` | — | pendiente |

---

## Voz

### `leo-daemon` → `leo-daemon`

Proceso de fondo. Carga `~/.config/leo-ai/config.toml` (plantilla: `config/leo-ai.example.toml`), monta captura/player, STT, LLM y TTS, y escucha el socket UNIX de `leo-ipc`.

El LLM del daemon es el de `[llm]` en el toml (por defecto Ollama), **no** el de Postgres. STT: Grok si hay `XAI_API_KEY`; si no, la voz no se transcribe. TTS: `NullTts` (beep) hasta que haya Piper/Kokoro.

### `leo-ctl` → `leo-ctl`

CLI del daemon. Manda un JSON por el socket y pinta el estado.

```
leo-ctl status | listen | stop | speak hola | shutdown
```

`listen` es hoy la forma de “despertar” a Leo (el wake word aún no está cableado).

### `leo-ipc`

Protocolo JSON por socket UNIX (`$XDG_RUNTIME_DIR/leo-ai.sock`, o `/tmp/leo-ai.sock`).

Órdenes: `status`, `listen`, `stop`, `speak`, `shutdown`. Lo usan `leo-daemon` (servidor) y `leo-ctl` (cliente).

### `leo-core`

Máquina de sesión de voz: estados `idle → listening → recording → transcribing → thinking → speaking`.

- Wake (o `leo-ctl listen`) abre la escucha.
- VAD cierra la frase; STT transcribe; LLM responde; TTS habla.
- Barge-in: si hablas mientras Leo habla, corta la reproducción.

No sabe de Pulse ni de HTTP: recibe traits (`WakeSpotter`, `SttEngine`, `LlmEngine`, `TtsEngine`) y frames de `leo-audio`.

### `leo-audio`

Captura y reproducción por Pulse/PipeWire (`libpulse-simple`).

- Captura a 48 kHz, resample a 16 kHz (`ML_RATE`) para VAD/STT.
- Player para PCM del TTS.
- RMS y beep de fallback.
- El `build.rs` enlaza las `.so` versionadas de Pulse.

### `leo-vad`

WebRTC VAD a 16 kHz, frames de 20 ms, hangover de silencio para decidir cuándo acabó la frase. Eventos: `Speech`, `Silence`, `SpeechEnded`.

### `leo-wake`

Trait `WakeSpotter` + `NoopWake`. El detector real (rustpotter) no está cableado: candle-core 0.2 no compila en el Rust actual. La activación es `leo-ctl listen` (atajo de teclado del escritorio).

### `leo-stt`

Trait `SttEngine`. Implementación: **Grok Speech-to-Text** (`GrokStt`, necesita `XAI_API_KEY`). `NullStt` no inventa texto. Convierte PCM a WAV para la API.

### `leo-tts`

Trait `TtsEngine`. Hoy `NullTts`: un beep cuya duración escala un poco con el texto. Aquí irán Piper/Kokoro; el altavoz sigue en `leo-audio`.

---

## Quién usa a quién

```
leo            → leo-store, leo-llm, leo-tools
leo-telegram   → leo-store, leo-llm, leo-tools
leo-desktop    → leo-store, leo-llm, leo-tools, leo-ipc
leo-tools      → leo-llm, leo-tools-{files,shell,system,weather,appflowy,github,google,home-assistant}
leo-store      → leo-llm
leo-daemon     → leo-core, leo-ipc, leo-llm, leo-stt, leo-tts, leo-wake, leo-audio
leo-ctl        → leo-ipc
leo-core       → leo-audio, leo-vad, leo-wake, leo-stt, leo-tts
leo-vad        → leo-audio
leo-tts        → leo-audio
```
