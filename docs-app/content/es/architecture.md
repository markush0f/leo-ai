# Visión general de arquitectura

Ira es un workspace de Cargo con `crates/ira-*`, crates de integración bajo
`crates/tools/*`, servicios Python/Node bajo `services/` y el backend Tauri
en `desktop/src-tauri`.

## Dos caminos de ejecución

```text
Chat
  TUI / Telegram / React → Tauri
  React (navegador) → ira-server
          │                 │
          ├─────────────────┘
          ▼
      ira-api → ira-store → catálogo PostgreSQL
          │
          ▼
      ira-tools → crates de integración
          │
          ▼
       ira-llm → API HTTP del proveedor (Ollama, Grok, GPT, Claude)

Voz
  ira-ctl / Tauri → ira-ipc → ira-daemon
                                 │
                             ira-core
                                 │
             captura → VAD → STT → LLM → TTS → reproducción
```

Las superficies de chat comparten el catálogo de PostgreSQL (proveedores,
modelos, motores, ajustes) y el historial de conversaciones. El navegador usa
`ira-server` para llegar a Ollama sin CORS. Los crates de voz siguen en el
workspace pero no se exponen en TUI, Telegram, escritorio ni navegador hasta
que ese trabajo esté planificado.

## Aplicaciones

| Crate / directorio | Entry point | Responsabilidad |
| --- | --- | --- |
| `ira-tui` | `ira` | Chat Ratatui y editor de catálogo. `app` posee el estado; `input` y `slash` enrutaban la entrada; `settings` y `ui` editan y pintan. |
| `ira-telegram` | `ira-telegram` | Long polling, allowlist, historial por sesión y herramientas compartidas. El enrutamiento de librería va separado del transporte HTTP `tg`. |
| `desktop/` | `npm run desktop` | Shell React y comandos nativos para chat y edición del catálogo. |
| `ira-api` | librería | DTOs compartidos de catálogo y chat usados por Tauri y `ira-server`. |
| `ira-server` | `ira-server` | HTTP `/api` para navegador y otras máquinas. Llama a Ollama desde el proceso del servidor. |
| `ira-daemon` | `ira-daemon` | Proceso de voz (diferido). Carga catálogo y motores, sirve IPC Unix. |
| `ira-ctl` | `ira-ctl` | CLI de voz (diferida). Solo habla con el daemon por el socket IPC. |
| `ira-pgjson` | `ira-pgjson` | Vuelca a JSON las tablas de una conexión PostgreSQL (esquema, claves, relaciones, filas). |

## Capas compartidas

- **`ira-store`** — persistencia del catálogo en PostgreSQL: `Snapshot`
  (copia en memoria), `DbOp` (ediciones), conversaciones, secretos. Ver
  [ira-store](#/store).
- **`ira-llm`** — transporte de proveedores: un `Client`, tres adaptadores
  de protocolo. Ver [ira-llm](#/llm).
- **`ira-tools`** — registro de herramientas y bucle modelo↔herramienta. Ver
  [ira-tools](#/tools).
- **`ira-core`** — máquina de estados de sesión de voz. Ver
  [Capa de voz](#/voice).
- **`ira-ipc`** — protocolo de control del daemon. Ver [ira-ipc](#/ipc).
- **`ira-mcp` / `ira-code` / `ira-engine`** — cliente de servidores MCP,
  utilidades de código y adaptadores de motores (p. ej. el cliente Colibri)
  usados por la capa de herramientas y los servicios.

## Servicios (`services/`)

| Servicio | Rol |
| --- | --- |
| `ira-realtime` | Puente de voz realtime en Python (WebSocket, 16 kHz). Llama a `ira-server` por HTTP con el token bearer; da voz al navegador fuera del daemon diferido. Lo arranca `scripts/start-ira-realtime.sh`. |
| `projects-api` | API de proyectos/tareas expuesta tras el gateway. |
| `gateway` | Front Caddy del tablero de servicios en `127.0.0.1:8790`. |

## Frontera escritorio / HTTP

- `src/App.tsx`: estado de conversación, burbujas, tema, catálogo y arranque de servicios del host.
- `src/Catalog.tsx`: borradores locales de formulario y operaciones de catálogo.
- `src/api.ts`: invocación Tauri, o `fetch` a `ira-server` fuera del webview.
- `src/types.ts`: DTOs del frontend y operaciones etiquetadas espejadas por `ira-api`.
- `src-tauri/src/lib.rs`: comandos Tauri finos sobre `ira_api::App`.
- `crates/ira-server`: HTTP `/api` sobre el mismo `App`. Bind por defecto
  `127.0.0.1:8787`. `GET/POST /api/services` informa y arranca servicios de
  Compose.

Mantén sincronizados nombres de campos, etiquetas de operación y DTOs de
`ira-api`. El navegador nunca llama a Ollama; `ira-server` sí.

## Fronteras de seguridad

- Los DTOs del escritorio exponen *estado* de credenciales, nunca valores de
  claves. No pases filas internas de la base de datos a través de la frontera
  del frontend.
- `ira-server` protege `/api` opcionalmente con bearer token
  (`IRA_HTTP_TOKEN`, si no `~/.ira/http.token`); el proxy de Vite inyecta el
  token en desarrollo.
- El acceso a bases de datos desde el modelo pasa por el contenedor **local**
  de MCP Toolbox con grants de solo lectura, nunca por una conexión `sqlx`
  directa.
- `Context::resolve` en la capa de herramientas resuelve rutas relativas pero
  no ofrece sandbox de sistema de archivos: las herramientas corren con los
  permisos del usuario.
