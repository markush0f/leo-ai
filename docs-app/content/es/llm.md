# ira-llm: transporte de proveedores

`ira-llm` es el transporte HTTP independiente del proveedor para modelos de
chat. No posee estado de conversación ni ejecuta herramientas: transporta
llamadas a herramientas.

## Tipos core

| Tipo | Contrato |
| --- | --- |
| `Client` | Ajustes del proveedor + un cliente `reqwest` reutilizable. Se construye desde la fila activa del catálogo con `Snapshot::client()`. |
| `ChatRequest` | Texto de sistema, historial de mensajes, esquemas de herramientas, opciones de razonamiento. `with_history(system, history)` es el constructor estándar. |
| `ChatResponse` | Texto final del asistente más cualquier llamada a herramienta, con info de uso del modelo. |
| `LlmError` | Fallos de transporte/protocolo, incluidos `MissingKey(var)` (se muestra como "falta api key") y `ToolLoop` de la capa de herramientas. |
| `protocol` | Constructores y parsers de payloads separados para que los tests corran offline. |

## Adaptadores

| Adaptador | Proveedores | Protocolo wire |
| --- | --- | --- |
| `openai_compat` | Grok, GPT | Mensajes y tool calls compatibles con OpenAI (`/chat/completions`). |
| `ollama` | Ollama | `/api/chat` nativo más descubrimiento de modelos (`/api/tags`). |
| `claude` | Claude | `/v1/messages` de Anthropic con traducción de tool-use. |
| `codex` | Codex (backend de ChatGPT) | Flujo de login estilo dispositivo OAuth gestionado por `begin_codex_login` / `finish_codex_login` en `ira-api`. |

La identidad y defaults del proveedor viven en `ira-llm/src/providers/`.
Añadir un proveedor significa extender ese módulo, el dispatch del cliente y
un adaptador de protocolo con tests offline de payload/respuesta.

## Streaming

`Client::chat_stream` expone streaming por proveedor (`chat_openai_stream`,
`chat_ollama_stream`, `chat_claude_stream`). Los deltas se empujan por un
callback sink; `ira-api` los reenvía al navegador como NDJSON (ver
[Referencia de la API HTTP](#/http-api)).

## Entorno

- `load_dotenv()` lee `.env` al iniciar el proceso **sin sobreescribir**
  variables ya exportadas. Todos los binarios (`ira`, `ira-daemon`,
  `ira-telegram`, `ira-server`) lo llaman primero.
- Una API key vacía cae a la variable de entorno del proveedor.

## Puente bloqueante (voz)

El daemon de voz envuelve el cliente async en `BlockingLlm`, que hace de
puente a `tokio::runtime::Handle::block_on`. `GrokStt` hace lo mismo. Reglas:

- Llama solo desde un hilo bloqueante dedicado mientras el runtime sigue
  activo.
- Nunca llames `block_on` desde dentro de una tarea async: entra en pánico.
- Las peticiones LLM de voz contienen la entrada actual del usuario más el
  prompt de sistema de voz, no historial persistente.
