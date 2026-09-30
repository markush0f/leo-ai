# Crates de integración de herramientas

Los crates de integración viven en `crates/tools/*` y siguen el patrón
`spec()` + `run(...)` documentado en
[ira-tools](#/tools). `ira-tools/src/catalog.rs` adapta argumentos JSON,
resuelve rutas con `Context` y registra las implementaciones.

| Sufijo del crate | Operaciones | Requisitos de registro |
| --- | --- | --- |
| `files` | Leer, escribir, listar, buscar, copiar, mover, borrar | Siempre registrado. |
| `shell` | Comandos y scripts | Siempre registrado; timeout y salida capturada. |
| `system` | Procesos, aplicaciones, URLs, notificaciones, portapapeles | Siempre registrado; las utilidades del host determinan disponibilidad. |
| `weather` | Tiempo actual y pronóstico | Open-Meteo; sin API key. |
| `appflowy` | Crear, recuperar, buscar, actualizar, borrar páginas | Configuración y credenciales del servidor AppFlowy. |
| `github` | Issues y pull requests | `GITHUB_TOKEN` o `GH_TOKEN`. |
| `google` | Calendarios y eventos | `GOOGLE_ACCESS_TOKEN` o `GOOGLE_API_KEY`. |
| `home-assistant` | Estados de entidades y service calls | URL del servidor y token. |
| `db` | SQL y descubrimiento de esquema vía MCP Toolbox local | `docker compose up -d toolbox` y `MCP_TOOLBOX_URL=http://127.0.0.1:5000`. |
| `notion`, `spotify` | Ninguna | Crates placeholder, no registrados. |

## Notas de comportamiento

- **shell**: la salida se captura y acota; comandos largos cumplen el timeout
  configurado y la herramienta devuelve lo que tiene.
- **system**: depende de capacidades (notificaciones requieren un servicio de
  notificaciones operativo; portapapeles depende de las utilidades de la
  sesión).
- **appflowy**: el cliente cachea un token de auth y reintenta una vez tras
  un `401` refrescándolo; cubre operaciones de workspace, carpeta y página.
- **github / google / home-assistant**: clientes tipados finos sobre las APIs
  HTTP del host; si falta credencial la herramienta simplemente no se
  registra y el modelo nunca la ve.
- **db**: nunca conecta directo — cada llamada se proxifica por el contenedor
  Toolbox local para que las credenciales queden fuera del alcance del
  modelo. La CLI `ira-pgjson` puede volcar tablas de cualquier conexión a JSON
  (esquema, claves, relaciones, filas) para inspección offline.

## Helpers de argumentos

`ira-tools/src/args.rs` aporta los validadores compartidos por todos los
crates: `require_str`, `opt_str` y `opt_str_list`. Úsalos en herramientas
nuevas para que los errores de argumentos sean uniformes (se convierten en
contenido JSON de error que el modelo puede leer).
