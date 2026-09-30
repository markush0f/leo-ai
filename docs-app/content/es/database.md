# Esquema de PostgreSQL

El catálogo vive en PostgreSQL 16 (servicio Compose en el puerto `5439`,
`postgres://ira:ira@127.0.0.1:5439/ira`). Esquema base:
`deploy/postgres/init.sql`; cambios aditivos bajo
`deploy/postgres/migrations/` registrados en `schema_migrations`.

## Tablas

### providers

| Columna | Notas |
| --- | --- |
| `id UUID PK` | IDs fijos sembrados para grok/gpt/ollama/claude/codex. |
| `name TEXT UNIQUE` | Nombre de visualización y búsqueda. |
| `kind TEXT` | Selector de adaptador: `grok`, `gpt`, `ollama`, `claude`, `codex`. |
| `base_url TEXT` | Endpoint del proveedor (p. ej. `http://127.0.0.1:11434` en Ollama). |
| `api_key TEXT` | Hueco legacy/plano; prefiere las columnas cifradas. |
| `api_key_ciphertext BYTEA`, `api_key_nonce BYTEA` | Clave cifrada (master key `IRA_MASTER_KEY` o `.ira/master.key`). |
| `created_at` | Auditoría. |

### models

`id`, `provider_id → providers ON DELETE CASCADE`, `name` (único por
proveedor), `effort CHECK IN ('low','medium','high','xhigh')`. `effort`
alimenta el nivel de razonamiento aplicado a las peticiones de chat.

### engines

Motores de voz: `role CHECK IN ('stt','tts','wake')`, `kind`, `name` (único
por rol), `provider_id` opcional, `config JSONB` (p. ej.
`{"model": "/ruta/m.onnx"}` para wake). `settings` referencia el motor
elegido por rol.

### settings (singleton, `id = 1`)

Punteros al modelo y motores activos, `system_prompt`,
`voice_system_prompt`, dispositivos de audio (`audio_source`/`audio_sink`,
por defecto `@DEFAULT_SOURCE@`/`@DEFAULT_SINK@`), ajuste VAD
(`vad_hangover_ms` 500), barge-in (`barge_in` bool + `barge_in_rms` 0.035),
`stt_language` (`es`), `thinking`, `tools_enabled`, `tools_mutate`,
`active_conversation_id`, `telegram_token`, `telegram_allow_users BIGINT[]`.

### conversations / messages

- `conversations`: `channel CHECK IN ('local','telegram','voice')`,
  `external_id`, `title`, `archived_at`. Índices únicos parciales garantizan
  una fila voice viva y una fila viva por `external_id` de Telegram.
- `messages`: `role CHECK IN ('user','assistant','tool','error')`,
  `content`, `tool_call_id`, `name`, `tool_calls JSONB`, `model_id` (qué
  modelo respondió en filas assistant), `created_at`; indexado por
  `(conversation_id, created_at)`.

### secrets

`key TEXT PK`, `value TEXT`, `updated_at`. Exportados al entorno del proceso
al arrancar (`apply_secrets_to_env`) sin sobreescribir variables ya
exportadas.

### database_connections

Bases de datos externas registradas para el puente MCP Toolbox:
host/puerto/base/usuario, `ssl_mode CHECK IN ('disable','prefer','require',
'verify-ca','verify-full')`, `enabled`, `password_ciphertext`/
`password_nonce` cifrados (un CHECK los mantiene emparejados), último test
(`last_test_ok`, `last_test_error`, `last_tested_at`). El panel web renderiza
las filas activas en `.ira/toolbox` para el hot reload de Toolbox.

### host_services

Definición del tablero de servicios: `id TEXT PK` (nombre del servicio
Compose), `name`, `required`, `position`. Sembrado con `postgres`, `toolbox`,
`ira-realtime`, `colibri`.

### mcp_manager

Servidores MCP externos: `transport CHECK IN ('stdio','streamable_http')`
con CHECKs emparejados (`stdio` ⇒ `command`, `streamable_http` ⇒ `url`),
`args JSONB`, `env JSONB`, `enabled`. Los carga al construir el registry de
herramientas `ira_tools::attach_configured`.

## Catálogo sembrado

UUIDs fijos siembran los proveedores grok (`https://api.x.ai/v1`), gpt
(`https://api.openai.com/v1`), ollama (`http://127.0.0.1:11434`), claude
(`https://api.anthropic.com/v1`) y codex
(`https://chatgpt.com/backend-api/codex`) con algunos modelos por defecto
(grok es el activo inicial), las filas de motores STT/TTS/wake y la fila
singleton `settings` con prompts de sistema en español.

## Reglas de migración

- `init.sql` es el baseline de estado actual para bases de datos nuevas.
- Los cambios aditivos salen como archivos numerados en
  `deploy/postgres/migrations/`, registrados en `schema_migrations`
  (`version INT PK`, `applied_at`).
- `store::migrate` corre en cada arranque del proceso; las migraciones deben
  ser idempotentes con las filas sembradas (`ON CONFLICT DO NOTHING`).

## Notas operativas

- Los usuarios de base de datos expuestos al modelo (vía Toolbox) deben tener
  grants de **solo lectura**; el default de sesión no es una frontera de
  autorización.
- Usa `host.docker.internal` dentro de contenedores para PostgreSQL en el
  host de Docker.
- Define `IRA_UID`/`IRA_GID` cuando Ira escriba archivos de runtime como un
  usuario distinto de `1000:1000`.
