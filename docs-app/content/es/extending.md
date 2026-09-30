# Extender Ira

## Herramienta nueva

1. Crea `crates/tools/ira-tools-<nombre>` con un módulo por operación, cada
   uno emparejando `spec()` (JSON Schema para el modelo) con `run(...)`
   (ejecución async tipada). Usa los helpers de `args` (`require_str`,
   `opt_str`, `opt_str_list`) para que los errores de argumentos sean
   uniformes.
2. Adapta y registra en `ira-tools/src/catalog.rs`: convierte argumentos
   JSON, resuelve rutas con `Context`, filtra por credenciales (una
   herramienta sin configurar no debe registrarse, en lugar de fallar al
   llamar).
3. Las herramientas que tocan bases de datos deben pasar por `ira-tools-db` y
   el contenedor MCP Toolbox local — nunca una conexión `sqlx` directa del
   modelo.
4. Añade tests offline del spec y de la conversión de argumentos.

## Proveedor LLM nuevo

1. Añade identidad y defaults del proveedor en `ira-llm` (módulo
   `providers`) y la rama de dispatch del cliente.
2. Implementa un adaptador de protocolo (constructores de payload + parsers
   de respuesta en `protocol`) para el formato wire del proveedor, incluida
   la traducción de tool calls si las soporta.
3. Añade streaming (`chat_<nombre>_stream`) si el proveedor lo permite.
4. Siembra la fila del catálogo (`providers.kind`) vía migración o deja que
   el panel la cree; una key vacía cae a una variable de entorno.
5. Añade tests offline de payload/respuesta; los fallos del proveedor deben
   salir como `LlmError`, nunca pánicos.

## Motor de voz nuevo

1. Implementa el trait síncrono correspondiente — `SttEngine` (devuelve
   transcripción) o `TtsEngine` (devuelve PCM mono + sample rate) — en
   `ira-stt`/`ira-tts`. Los puentes HTTP async van por
   `Handle::current().block_on` y solo pueden llamarse desde el hilo
   bloqueante del motor.
2. Añade la fila del motor (`role`, `kind`, `config`) y selecciónala en el
   catálogo (`SetEngine`).
3. Conecta la construcción en `ira-daemon` (patrón `build_stt`: match de
   kind + comprobación de credencial + fallback `Null*` con aviso).
4. Mantén la reproducción en `ira-audio`; la máquina de estados de sesión es
   independiente del motor.

## Operación de catálogo nueva

1. Añade la variante a `ira-store::DbOp` y persiste en `apply` (que recarga
   y devuelve el `Snapshot`).
2. Expón el resultado por DTOs de `ira-api` si el frontend lo lee.
3. Refleja el tag en `desktop/src/types.ts` — el discriminante `op` en
   snake_case debe coincidir exacto.
4. Conecta la UI (`Catalog.tsx` borradores → `onOp`); la pantalla de
   ajustes del TUI solo si la edición debe estar disponible ahí.
5. `ira-server` reenvía `Op` genéricamente; no necesita cambio por operación.

## Cambio de esquema nuevo

1. Añade un archivo numerado bajo `deploy/postgres/migrations/`; mantenlo
   idempotente (`IF NOT EXISTS`, `ON CONFLICT DO NOTHING`) y fusiona el
   estado actual en `init.sql` para bases nuevas.
2. Registra la versión en `schema_migrations` tal como la aplica
   `store::migrate`.
3. Nunca elimines columnas en el mismo release en que dejas de escribirlas.

## Artículo nuevo para esta app de documentación

1. Escribe `docs-app/content/en/<slug>.md` y `docs-app/content/es/<slug>.md`.
2. Añade una entrada a `docs-app/content/index.json` (`slug`, `section`,
   `title` + `summary` localizados).
3. `npm run build` en `docs-app/` genera `desktop/public/docs/`; sin cambios
   de código.

## Comandos de verificación

```sh
cargo fmt --all -- --check
cargo test -p ira-core -p ira-llm -p ira-tools -p ira-api -p ira-server
cargo doc --workspace --no-deps
cd desktop && npm run build
cd docs-app && npm run build
```
