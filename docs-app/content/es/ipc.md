# ira-ipc: protocolo de control del daemon

`ira-ipc` define un protocolo JSON de una petición/una respuesta sobre un
socket de dominio Unix entre `ira-ctl` (o el escritorio) e `ira-daemon`.

## Transporte

- Ruta del socket: `$XDG_RUNTIME_DIR/ira-ai.sock`, con fallback a
  `/tmp/ira-ai.sock` (`ira_ipc::socket_path`).
- Los mensajes son JSON delimitado por nueva línea. El cliente conecta,
  escribe una línea de petición, hace cierre parcial y lee una línea de
  respuesta.
- El bind elimina primero la entrada de socket existente, así que el llamador
  debe asegurar que ningún otro daemon use esa ruta.
- El cliente no impone **ningún timeout**; `IpcError::NotRunning(path)`
  significa que el socket no existe (daemon no corriendo). Envuelve la
  llamada acotar la espera.

## Peticiones

Enum etiquetado con discriminador `cmd`, en snake_case:

| Comando | JSON | Significado |
| --- | --- | --- |
| `Status` | `{"cmd":"status"}` | Último estado de sesión publicado. |
| `Listen` | `{"cmd":"listen"}` | Entrar en escucha (hotkey típico de escritorio). |
| `Stop` | `{"cmd":"stop"}` | Cancelar escucha o reproducción. |
| `Speak` | `{"cmd":"speak","text":"Hola"}` | Sintetizar; el fallback es un beep sin motor TTS. |
| `Shutdown` | `{"cmd":"shutdown"}` | Pedir al daemon que salga. |

## Respuesta

```json
{ "ok": true, "state": "listening", "message": "opcional" }
```

`message` se omite cuando es `None`. `ira-ctl` imprime `state` (más
` — message`) y sale con `1` si falla el transporte o `ok: false`.

## Semántica de propiedad

El daemon posee las transiciones de sesión. Un envío con éxito significa que
el comando fue **aceptado**, no que STT/LLM/TTS terminaron. `ira-ctl` no
carga el catálogo, ni llama proveedores, ni abre dispositivos de audio: un
proceso, un comando.
