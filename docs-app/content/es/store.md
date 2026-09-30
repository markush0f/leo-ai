# ira-store: persistencia del catálogo

`ira-store` posee todo el estado en PostgreSQL: proveedores, modelos, motores
STT/TTS/wake, ajustes (incluido el modelo activo y parámetros de voz),
secretos, conexiones de base de datos, servicios del host, servidores MCP e
historial de conversaciones. El esquema base vive en
`deploy/postgres/init.sql`; los cambios aditivos versionados bajo
`deploy/postgres/migrations/` se registran en `schema_migrations`.

## Snapshot

`Snapshot` es una copia en memoria del catálogo y los ajustes, incluidas las
credenciales internas de proveedor.

- `store::connect(url)` abre el pool; `store::migrate(pool)` aplica esquema y
  migraciones.
- `store::load(pool)` devuelve un `Snapshot` nuevo.
- `Snapshot::client()` construye el `Client` de `ira-llm` del modelo activo
  sin E/S de red; `store::client_with_pool(snap, pool)` es la entrada
  compartida por TUI, API y daemon.
- `snap.active_model()`, `snap.engine(role)`, `snap.reasoning_effort()`
  exponen filas seleccionadas; `apply_reasoning(&mut ChatRequest)` copia
  thinking/effort a la petición.
- Las conversaciones se cargan aparte (`ensure_local`, `context_messages`,
  `append_message`), no dentro del `Snapshot`.

## DbOp — el contrato de edición

Toda mutación del catálogo es una variante `DbOp`. `store::apply(pool, op)`
persiste una operación y recarga el catálogo, así las superficies siempre
pintan el estado post-escritura. Variantes (`crates/ira-store/src/lib.rs`):

| Grupo | Operaciones |
| --- | --- |
| Activación | `ActivateProvider`, `ActivateModel` |
| Proveedores | `NewProvider`, `RenameProvider`, `DeleteProvider`, `SetKind`, `SetApiKey`, `SetBaseUrl` |
| Modelos | `NewModel`, `RenameModel`, `DeleteModel`, `SetModelEffort` |
| Prompts | `SetSystem`, `SetVoiceSystem` |
| Voz | `SetEngine`, `SetVoiceAudio`, `SetVad`, `SetSttLanguage` |
| Comportamiento chat | `SetThinking`, `SetToolsEnabled`, `SetToolsMutate` |
| Telegram | `SetTelegram` (token + allowlist) |
| Secretos | `SetSecret` |

`DbOp` se serializa con tag `op` en snake_case. La unión `Op` del frontend en
`desktop/src/types.ts` refleja estos nombres 1:1 — ver
[Contrato compartido de DTOs y Op](#/dto-contract).

## Secretos y cifrado

- La tabla `secrets` guarda pares `key/value`; `apply_secrets_to_env(pool)`
  los exporta al entorno del proceso al arrancar (sin sobreescribir variables
  ya exportadas).
- Las API keys de proveedor y contraseñas de base de datos se guardan
  cifradas: `api_key_ciphertext`/`api_key_nonce` en `providers`,
  `password_ciphertext`/`password_nonce` en `database_connections`. La master
  key viene de `IRA_MASTER_KEY`; si no está definida, Ira crea
  `.ira/master.key` al primer uso y la reutiliza.
- Una API key vacía cae a la variable de entorno del proveedor a través de
  `ira-llm` (p. ej. `XAI_API_KEY`).
- Los DTOs del escritorio exponen estado de credenciales, no valores; nunca
  pases filas crudas a través de la frontera del frontend.

## Descubrimiento de Ollama

`sync_ollama_providers(pool)` descubre modelos de los proveedores Ollama
configurados (`/api/tags` nativo). Un proveedor no disponible se omite; los
errores de base de datos sí propagan. El TUI y la API lo llaman de forma
perezosa antes de un turno cuando el proveedor activo es Ollama.

## Conversaciones

- Las filas `conversations` llevan `channel` (`local`, `telegram`, `voice`)
  con índices únicos parciales: una conversación voice viva, una fila viva por
  `external_id` de Telegram.
- Las filas `messages` llevan `role` (`user`, `assistant`, `tool`, `error`),
  `tool_call_id` opcional, `name`, `tool_calls` JSONB y `model_id` (qué modelo
  produjo el texto del asistente).
- `context_messages(pool, id, limit)` devuelve la ventana reciente con la que
  se construye el historial de un `ChatRequest`.
- `NewMessage::user/assistant/error` son los constructores de todas las
  superficies; los errores se persisten como mensajes `role='error'` para que
  el historial muestre los turnos fallidos.

## Semántica de fallos

- Un fallo de conexión al arrancar el daemon devuelve un error con la pista de
  arrancar la base: `docker compose up -d`.
- `apply` devuelve el nuevo `Snapshot` con éxito; los errores SQL propagan
  (el TUI los muestra; HTTP los mapea a `{"error": ...}` con 500).
- Transcripciones de voz que no se guardan registran un aviso (`voz no
  guardada`) sin romper la sesión: la persistencia es fire-and-forget en el
  hilo `ira-events` del daemon.
