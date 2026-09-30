# Capa de voz: ira-core y motores

La pila de voz está diferida como superficie de producto: `ira-daemon` y sus
crates siguen en el workspace pero no conectan con TUI, Telegram, escritorio
ni navegador. El camino de voz del navegador hoy pasa por el servicio
`ira-realtime`.

## ira-core: máquina de estados

```text
idle → listening → recording → transcribing → thinking → speaking
```

- `Session` posee las transiciones y el audio acumulado del enunciado.
  Devuelve valores `Action` para efectos secundarios y `SessionEvent` para
  observadores (`Transcript`, `Reply`, `BargeIn`, ...).
- `spawn_engine(config, wake, stt, llm, tts)` ejecuta las acciones en un hilo
  dedicado con proveedores inyectados. Las llamadas a proveedores son
  **síncronas**, así que los comandos IPC esperan mientras se ejecutan.
- El silencio del VAD cierra un enunciado. Transcripciones o respuestas vacías
  devuelven la sesión a `idle`.
- Durante la reproducción, la detección de barge-in configurada puede parar
  la reproducción y reabrir la escucha. Los límites por frames asumen bloques
  de 20 ms; el daemon convierte `vad_hangover_ms` en
  `hangover_frames = ms / 20` (mínimo 1).

## Crates de audio y habla

| Crate | Contrato y comportamiento |
| --- | --- |
| `ira-audio` | Captura/reproducción PulseAudio a 48 kHz; la captura emite frames mono a 16 kHz. Una cola de captura de ocho frames descarta frames nuevos cuando está llena. |
| `ira-vad` | WebRTC VAD a 16 kHz con umbrales de mínimo de voz y hangover de silencio. El detector permanece en su hilo propietario. |
| `ira-wake` | Punto de extensión `WakeSpotter`; el cargador devuelve hoy `NoopWake`. La ruta del modelo se configura en la fila de motor `wake` (`config.model`). |
| `ira-stt` | `SttEngine` síncrona; `GrokStt` sube WAV PCM16 mono; `NullStt` no devuelve transcripción. |
| `ira-tts` | `TtsEngine` síncrona que devuelve PCM mono con su sample rate; `NullTts` genera un tono (el fallback de `speak` del daemon es un beep). |

`GrokStt` y el `BlockingLlm` del daemon puentean HTTP async con
`tokio::runtime::Handle::block_on` — llámalos solo desde hilos bloqueantes.
Las peticiones LLM de voz contienen la entrada actual del usuario y el prompt
de sistema de voz (ajuste `voice_system_prompt`), no historial persistente.

## Cableado del ira-daemon

Al arrancar (`crates/ira-daemon/src/main.rs`):

1. Conecta y migra PostgreSQL, aplica secretos al entorno, importa la config
   TOML del archivo una vez si los ajustes siguen por defecto.
2. Carga el `Snapshot`, construye `EngineConfig` desde los ajustes de voz
   (`audio_source`, `audio_sink`, barge-in, hangover VAD).
3. Carga el modelo de wake (opcional), construye STT (kind `grok` con API
   key, si no `NullStt`), el `BlockingLlm` (o `NullLlm` si falta la key — el
   daemon avisa y continúa), `NullTts`.
4. `spawn_engine` arranca el hilo de sesión; `ensure_voice` crea la única
   conversación `voice` viva.
5. El hilo `ira-events` consume `SessionEvent`: transcripciones y respuestas
   se persisten en la conversación de voz (`persist_voice`, fire-and-forget).
6. Ata el socket IPC y sirve peticiones hasta el apagado.

`ira-ctl listen` es el destino previsto del hotkey; el daemon imprime la
pista al arrancar.
