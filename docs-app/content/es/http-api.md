# Referencia de la API HTTP

`ira-server` expone el mismo catálogo y motor de chat que el shell Tauri por
HTTP. Escucha en `127.0.0.1:8787` por defecto; `--bind` o `IRA_HTTP_BIND`
cambian la dirección (p. ej. `0.0.0.0:8787` para LAN). Sacarlo de loopback
sigue exigiendo token: toda petición debe autenticarse.

## Autenticación

Todas las rutas `/api` pasan por autorización bearer:

- Origen del token: `IRA_HTTP_TOKEN`, si no el archivo `~/.ira/http.token`
  (generado al primer arranque del servidor).
- Los clientes envían `Authorization: Bearer <token>`.
- El proxy de desarrollo de Vite inyecta el token automáticamente en `/api`.
- El mismo token lo usa el servicio `ira-realtime` para llamar a la API.

## Envolvente de respuesta

Los éxitos son DTOs JSON. Los errores son `{"error": "<mensaje>"}` con status
no-2xx (`400` para mensaje vacío, `500` para fallos internos). El helper
`http<T>()` del frontend convierte cualquier campo `error` en un `Error`
lanzado.

## Endpoints

Montados bajo `/api`:

| Método | Ruta | Body | Devuelve | Notas |
| --- | --- | --- | --- | --- |
| `GET` | `/api/health` | — | `{ok, db}` | `db` es una sonda de conectividad en vivo. |
| `GET` | `/api/snapshot` | — | `Snapshot` | DTO completo de catálogo + ajustes. |
| `POST` | `/api/apply` | `Op` | `Snapshot` | Aplica una edición del catálogo. |
| `GET` | `/api/services` | — | `Services` | Estado del tablero de servicios (Compose). |
| `POST` | `/api/services` | — | `Services` | Arranca los servicios marcados para boot. |
| `POST` | `/api/services/{id}` | `{action, port?, name?, description?}` | `Services` | `action` ∈ `start`, `stop`, `autostart`, `manual`, `port`, `meta`. |
| `GET` | `/api/chats` | — | `Conversation[]` | Conversaciones recientes. |
| `POST` | `/api/chats` | — | `Conversation` | Crea una. |
| `GET` | `/api/chats/{id}` | — | `Turn[]` | Transcripción completa. |
| `POST` | `/api/chats/{id}/messages` | `{text}` | `{text}` | Un turno de chat bloqueante; texto vacío → `400`. |
| `POST` | `/api/chats/{id}/messages/stream` | `{text}` | stream NDJSON | Turno en streaming, abajo. |
| `GET` | `/api/databases` | — | `DatabaseConnection[]` | Solo estado de credenciales, nunca valores. |
| `POST` | `/api/databases` | `DatabaseInput` | `DatabaseConnection` | Registra una conexión (contraseña cifrada). |
| `PUT` | `/api/databases/{id}` | `DatabaseInput` | `DatabaseConnection` | La actualiza. |
| `DELETE` | `/api/databases/{id}` | — | `{ok}` | La elimina. |
| `POST` | `/api/databases/{id}/test` | — | `DatabaseTest` | Conecta y registra `last_test_ok`/error. |
| `GET` | `/api/databases/{id}/json` | — | volcado JSON | Exportación completa vía `ira-pgjson` (esquema + filas). |
| `GET` | `/api/databases/{id}/schema` | — | volcado JSON | Exportación solo esquema. |
| `POST` | `/api/codex/login` | `{provider_id}` | `CodexLogin` | Inicia el login dispositivo de Codex. |
| `POST` | `/api/codex/login/{id}/finish` | — | `Snapshot` | Lo completa y recarga el catálogo. |

## Chat en streaming (NDJSON)

`POST /api/chats/{id}/messages/stream` devuelve
`Content-Type: application/x-ndjson` con `Cache-Control: no-cache` y
`x-accel-buffering: no`. Cada línea es un evento:

```json
{"type":"delta","text":"parcial"}
{"type":"reset"}
{"type":"done"}
{"type":"error","error":"mensaje"}
```

- `delta` — añade texto parcial del asistente (puede llegar repetidamente; el
  cliente fusiona).
- `reset` — descarta la respuesta acumulada y reinicia (cuando el bucle de
  herramientas reintenta sin herramientas).
- `done` — turno completo; la respuesta y el mensaje de usuario quedan
  persistidos.
- `error` — el turno falló; el error también se guarda como mensaje `error`.

En el servidor, el turno corre en una tarea generada que alimenta un canal
acotado (profundidad 32); un body descartado aborta el relé. Si el cliente se
desconecta a mitad del stream, la generación continúa hasta completarse y se
persiste.

## Raíz web estática

Cuando `desktop/dist` (autodetectado junto al binario o vía `IRA_WEB_ROOT`)
contiene `index.html`, el mismo servidor también sirve el frontend construido
— incluida esta app de documentación en `/docs/` — así un despliegue de una
sola máquina es un solo proceso.

## Concurrencia

Los turnos de chat se serializan por conversación con un lock en el proceso;
dos superficies en la misma conversación hacen cola en lugar de intercalarse.
