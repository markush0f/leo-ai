# Contrato compartido de DTOs y Op

Tres superficies (escritorio Tauri, navegador vía `ira-server`, TUI) leen y
editan el mismo catálogo. `ira-api` es el único punto de definición en Rust;
el frontend lo refleja en TypeScript. Mantenerlos sincronizados es un
contrato manual.

## Capas

```text
desktop/src/types.ts   ← espejo —   crates/ira-api/src/dto.rs
        │                                   │
   desktop/src/api.ts                   ira_api::App
   (invoke | fetch)                    (snapshot, apply, chat, …)
        │                        ┌──────────┴──────────┐
        └── comandos Tauri ──────┤ handlers ira-server │
                                 └─────────────────────┘
```

- `ira-api::App` posee las operaciones de catálogo que usan ambos
  transportes: `snapshot()`, `apply(Op)`, `chat()`, `chat_stream()`,
  `list_chats()`, servicios, bases de datos, login Codex.
- Los comandos Tauri de `desktop/src-tauri/src/lib.rs` son wrappers finos
  sobre el mismo `App`.
- Los handlers de `ira-server` son wrappers finos sobre el mismo `App` más la
  guarda bearer.

## DTO Snapshot

Copia del catálogo serializada para el frontend: proveedores, modelos,
motores, ajustes y flags de **estado** de credenciales (p. ej. "tiene key"),
nunca valores de claves. Las filas internas nunca cruzan la frontera; la
conversión a DTO ocurre en `ira-api/src/dto.rs`.

## Op — ediciones etiquetadas

`ira-store::DbOp` se serializa con tag `op` en snake_case; la unión `Op` del
frontend en `desktop/src/types.ts` lista las mismas variantes:
`activate_provider`, `activate_model`, `set_kind`, `set_system`,
`set_voice_system`, `set_api_key`, `set_base_url`, `new_provider`,
`new_model`, `rename_provider`, `rename_model`, `delete_provider`,
`delete_model`, `set_engine`, `set_stt_language`, `set_thinking`,
`set_model_effort`, `set_tools_enabled`, `set_tools_mutate`.

```ts
const op: Op = { op: "set_thinking", value: true };
const snap = await applyOp(op); // Tauri invoke("apply") o POST /api/apply
```

Ambos transportes devuelven el **Snapshot completo actualizado**, así el
frontend nunca parchea estado de forma optimista.

## Eventos de stream

`ChatStreamEvent` (`ira-api`) es la forma de cada línea NDJSON y del payload
de `Channel<ChatStreamEvent>` en Tauri:
`{type:"delta"|"reset"|"done"|"error"}`. Ver
[Referencia de la API HTTP](#/http-api).

## Reglas de sincronización al extender

1. Añade la variante en `ira-store/src/lib.rs` (`DbOp`) y persista en `apply`.
2. Expón el resultado por `ira-api` (DTO/`Op` si el frontend lo usa).
3. Refleja el tag en la unión `Op` de `desktop/src/types.ts`.
4. Conecta la UI en `Catalog.tsx` (los borradores son locales; las
   operaciones salen por `onOp`).
5. `ira-server` no cambia (reenvía `Op` genéricamente); el TUI solo si la
   edición debe estar disponible ahí.

Nombres de campos, ortografía de tags y opcionalidad deben coincidir exactos:
el tag es parte de la forma JSON y los desajustes fallan en deserialización
en runtime, no en compilación.
