# Frontend de escritorio y web

`desktop/` es una app React 19 + Vite que corre en dos anfitriones: dentro de
Tauri 2 (comandos nativos) y en el navegador (proxy de `/api` a
`ira-server`). El mismo código alimenta ambos.

## Estructura

| Archivo | Responsabilidad |
| --- | --- |
| `src/main.tsx` | Monta `App`, carga fuentes y estilos. |
| `src/App.tsx` | Shell: estado de conversación, burbujas, streaming, tema, orquestación de sheets, arranque de servicios, stage de voz. |
| `src/Catalog.tsx` | Sheet editor de catálogo: borradores locales; cada commit emite un `Op` por `onOp`. |
| `src/Databases.tsx` | Sheet de conexiones de base de datos (CRUD, test, activación en toolbox). |
| `src/Services.tsx` | Tablero de servicios del host: start/stop/autostart/port/meta por servicio Compose. |
| `src/VoiceStage.tsx` | Overlay de voz impulsado por los callbacks de `voice.ts`. |
| `src/components/` | `Composer`, `Message` (renderiza `Markdown`), `Field`, hooks de foco. |
| `src/Markdown.tsx` | Renderizado GFM con `react-markdown`, compilado a elementos React (sin `dangerouslySetInnerHTML`). |
| `src/api.ts` | Frontera de transporte (abajo). |
| `src/types.ts` | DTOs + unión `Op` espejados de `ira-api` (ver [dto-contract](#/dto-contract)). |
| `src/theme.ts` | Preferencia de tema guardada bajo `ira-theme`, aplicada con `data-theme` en `<html>`. |
| `src-tauri/` | Shell Rust: comandos finos sobre `ira_api::App`. |

## Split de transporte (api.ts)

`inTauri` (`"__TAURI_INTERNALS__" in window`) elige la ruta por función:

- **Tauri**: `invoke("snapshot")`, `invoke("apply", { op })`, streaming con
  `Channel<ChatStreamEvent>`.
- **Navegador**: `http<T>()` contra `/api` con `Authorization: Bearer` cuando
  hay `VITE_IRA_HTTP_TOKEN`. Los fallos de conexión lanzan un mensaje
  indicando que arranques `ira-server`.

Flujo de desarrollo (`npm run web` → `scripts/dev.mjs`): arranca `ira-server`
en `127.0.0.1:8787` y Vite en `5179`; el proxy de Vite inyecta el token
bearer desde `IRA_HTTP_TOKEN` o `~/.ira/http.token`. Los puertos fallan de
forma segura si están ocupados (`IRA_DEV_PORT` / `IRA_HTTP_PORT`).

## Comportamiento del chat

- `streamChat` consume líneas NDJSON fusionando `delta` en una sola burbuja
  del asistente (`mergeReply` maneja repeticiones fuera de banda); `reset`
  limpia el parcial; los errores reemplazan la burbuja visible por una de
  error.
- El shell mantiene un contador de ejecución (`chatRunRef`) para que un
  stream cancelado o tardío no mute estado más nuevo; el render se coalesce
  con `requestAnimationFrame`.
- Al arrancar: cuenta vacía → crea chat automáticamente; deep-link
  `?catalog` abre el catálogo; `?demo` activa modo demo;
  `?theme=light|dark` fuerza tema.
- Los servicios se sondean cada 4 s con `loadServices`.

## Camino de voz

`src/voice.ts` habla con el servicio **ira-realtime**, no con el daemon
diferido:

- URL base: `VITE_IRA_REALTIME`, por defecto
  `http://127.0.0.1:8790/realtime` (gateway Caddy → contenedor
  `ira-realtime`).
- Un WebSocket sube PCM del micrófono y baja reproducción + eventos
  (`transcript`, `reply`, `level`, `state`, `error`); el stage muestra
  transcripción del usuario, respuesta de Ira y fase.
- Sonda de salud en `/`; si el servicio está en modo eco
  (`IRA_REALTIME_MODE` ≠ `ira`) la voz se rechaza con la pista de arranque.
- Transcripciones y respuestas se persisten como turnos normales de la
  conversación: el servicio realtime llama a la API HTTP de chat de
  `ira-server`.

## Convenciones de accesibilidad

- Foco de sheets gestionado con `useSheetFocus` (atrapar + restaurar); el
  cajón del rail se marca `role="dialog"` en móvil.
- Las superficies de fondo reciben `inert` mientras un sheet/rail está aberto.
- El fin de una respuesta en streaming se anuncia por una región aria-live.
- Se respeta la preferencia de movimiento reducido (`useReducedMotion`).

## Acceso a la app de documentación

La app independiente de docs (`docs-app/`) se construye en
`desktop/public/docs/` y la sirve estáticamente Vite/Tauri e `ira-server`
(web root). El item **Documentación** del rail abre `/docs/` externamente
(`openExternal` → opener de Tauri o pestaña nueva).
