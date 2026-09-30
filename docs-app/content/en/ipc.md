# ira-ipc: Daemon Control Protocol

`ira-ipc` defines a one-request/one-response JSON protocol over a Unix domain
socket between `ira-ctl` (or the desktop) and `ira-daemon`.

## Transport

- Socket path: `$XDG_RUNTIME_DIR/ira-ai.sock`, falling back to
  `/tmp/ira-ai.sock` (`ira_ipc::socket_path`).
- Messages are newline-delimited JSON. A client connects, writes one request
  line, half-closes, and reads one response line.
- Binding removes the existing socket entry first, so the caller must ensure
  no other daemon is using the path.
- The client imposes **no timeout**; `IpcError::NotRunning(path)` means the
  socket is missing (daemon not running). Wrap the call to bound the wait.

## Requests

Tagged enum with a `cmd` discriminator, snake_case:

| Command | JSON | Meaning |
| --- | --- | --- |
| `Status` | `{"cmd":"status"}` | Latest published session state. |
| `Listen` | `{"cmd":"listen"}` | Enter listening (typical desktop hotkey). |
| `Stop` | `{"cmd":"stop"}` | Cancel listen or playback. |
| `Speak` | `{"cmd":"speak","text":"Hello"}` | Synthesize; fallback is a beep when no TTS engine. |
| `Shutdown` | `{"cmd":"shutdown"}` | Ask the daemon to exit. |

## Response

```json
{ "ok": true, "state": "listening", "message": "optional" }
```

`message` is omitted when `None`. `ira-ctl` prints `state` (plus ` — message`)
and exits `1` on transport failure or `ok: false`.

## Ownership semantics

The daemon owns session transitions. A successful send means the command was
**accepted**, not that STT/LLM/TTS finished. `ira-ctl` does not load the
catalog, call providers, or open audio devices — one process, one command.
