# ira-tools: orquestación de herramientas

`ira-tools` registra herramientas para el modelo y ejecuta el bucle
modelo↔herramienta. Todas las superficies de chat (TUI, Telegram, `ira-api`)
usan el mismo registry y la misma entrada `chat`.

## Ciclo de vida del Registry

1. `Registry::from_env()` captura el contexto de ejecución (directorio de
   trabajo, entorno) y registra las herramientas siempre disponibles.
2. `attach_configured(registry, &file_servers())` añade servidores MCP
   externos definidos en la tabla `mcp_manager` / archivos de configuración
   (transporte stdio o streamable-HTTP) a través de `ira-mcp`.
3. `registry.read_only()` da el subconjunto de solo lectura que usa Telegram;
   los ajustes `tools_enabled` / `tools_mutate` filtran herramientas de
   mutación.

## El bucle de chat

`ira_tools::chat(&client, request, &registry)` gestiona un turno de usuario:

```text
envía ChatRequest (+ esquemas de herramientas)
  └─ ¿el modelo devuelve tool calls?
       ejecútalas secuencialmente, añade resultados con su call ID,
       envía la siguiente petición al modelo con esos resultados
  └─ repite hasta una respuesta final de texto
```

- El bucle permite **ocho** respuestas del modelo. Si las ocho siguen
  pidiendo herramientas, devuelve `LlmError::ToolLoop`.
- Los fallos del proveedor abortan el turno de inmediato.
- Los fallos de herramienta **no** abortan: el error se convierte en
  contenido JSON para que el modelo pueda recuperarse y explicarlo.
- Un JSON de argumentos inválido se convierte hoy en objeto vacío antes de la
  validación individual de cada herramienta.

## Patrón de operación

Los módulos de integración emparejan dos funciones:

- `spec()` — el JSON Schema para el modelo.
- `run(ctx, args...)` — ejecución async tipada.

`ira-tools/src/catalog.rs` adapta argumentos JSON crudos, resuelve rutas con
el contexto capturado y registra las implementaciones.

## Contexto y seguridad

`Context::resolve` une rutas relativas al directorio de trabajo capturado. **No**
canonicaliza rutas ni provee sandbox de sistema de archivos: las herramientas
corren con los permisos del usuario. Trata `tools_mutate` y el subconjunto de
solo lectura de Telegram como comodidad, no como fronteras de seguridad.

## Herramientas de base de datos

SQL y descubrimiento de esquema pasan por `ira-tools-db` y el contenedor
**local** de MCP Toolbox (`db_list_tools`, `db_invoke`, `db_execute_sql`, …),
nunca por una conexión `sqlx` directa del modelo. Requisitos:

```sh
docker compose up -d postgres toolbox
MCP_TOOLBOX_URL=http://127.0.0.1:5000
```

El panel web escribe credenciales PostgreSQL cifradas en el catálogo y
renderiza las conexiones activas en `.ira/toolbox` para el hot reload de
Toolbox. Los usuarios de base de datos deben tener grants de solo lectura; el
default de sesión no es una frontera de autorización. Usa
`host.docker.internal` para PostgreSQL en el host de Docker.
