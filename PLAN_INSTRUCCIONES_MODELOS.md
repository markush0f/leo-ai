# Plan: instrucciones de Ira en Postgres

## Objetivo

Guardar en Postgres las instrucciones que definen cómo responde Ira. Aplicar las mismas reglas aunque cambie el proveedor o modelo, y permitir editarlas sin recompilar.

## Situación actual

- `settings.system_prompt` guarda el prompt general y `settings.voice_system_prompt` el de voz.
- `preferences.system_prompt`, en configuración local, puede sobrescribir el prompt general.
- Las reglas de memoria están fijas en `crates/ira-store/src/memories.rs`.
- Las reglas de herramientas, búsqueda web y presentación del modelo están fijas en `crates/ira-api/src/lib.rs`.

## Diseño propuesto

1. **Crear tabla `instructions`** con clave única, contenido, estado activo, orden y fecha de actualización. Bloques iniciales: `persona`, `memory`, `tools` y `web_search`.
2. **Migrar sin perder configuración**: conservar el prompt personalizado existente como `persona`; sembrar los demás bloques con sus instrucciones actuales. Resolver la precedencia de `preferences.system_prompt` para que el archivo local no siga sobrescribiendo cambios hechos en Postgres.
3. **Componer un único prompt por turno**: cargar bloques activos en orden, añadir contexto dinámico —modelo actual, herramientas disponibles y recuerdos relevantes— y enviarlo al modelo. El contenido de `memories` seguirá siendo datos, no instrucciones.
4. **Aplicarlo en todos los canales**: chat normal, streaming, Telegram, TUI y voz donde corresponda. Revisar reglas específicas de voz para no imponerle un estilo pensado para respuestas escritas.
5. **Permitir edición autenticada**: endpoints para consultar y actualizar bloques; añadir editor en ajustes con restauración de valores predeterminados. Validar claves y longitud del contenido.
6. **Verificar**: migración de instalaciones existentes, cambios efectivos sin reinicio, mismo comportamiento entre canales y ausencia de instrucciones duplicadas en el prompt.

## Límites

Las instrucciones propias de Grok, ChatGPT u otros proveedores no se copian ni gestionan aquí. Los esquemas y descripciones técnicas de las herramientas permanecen en código; Postgres guarda las reglas de comportamiento de Ira. Primera versión: instrucciones globales con variantes por canal, sin reglas distintas para cada modelo.
