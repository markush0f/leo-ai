# Voice Layer: ira-core and Engines

The voice stack is deferred as a product surface: `ira-daemon` and its crates
stay in the workspace but are not wired into TUI, Telegram, desktop, or the
browser. The browser voice path today goes through the `ira-realtime` service.

## ira-core: state machine

```text
idle → listening → recording → transcribing → thinking → speaking
```

- `Session` owns transitions and the accumulated utterance audio. It returns
  `Action` values for side effects and `SessionEvent` values for observers
  (`Transcript`, `Reply`, `BargeIn`, ...).
- `spawn_engine(config, wake, stt, llm, tts)` runs actions on a dedicated
  thread with injected providers. Provider calls are **synchronous**, so IPC
  commands wait while they run.
- VAD silence closes an utterance. Empty transcripts or replies return the
  session to `idle`.
- During playback, configured barge-in detection can stop playback and reopen
  listening. Frame-based limits assume 20 ms blocks; the daemon converts
  `vad_hangover_ms` into `hangover_frames = ms / 20` (minimum 1).

## Audio and speech crates

| Crate | Contract and behavior |
| --- | --- |
| `ira-audio` | PulseAudio capture/playback at 48 kHz; capture emits 16 kHz mono frames. An eight-frame capture queue drops new frames when full. |
| `ira-vad` | WebRTC VAD at 16 kHz with minimum speech and silence-hangover thresholds. The detector stays on its owning thread. |
| `ira-wake` | `WakeSpotter` extension point; the loader currently returns `NoopWake`. A model path can be configured through the `wake` engine row (`config.model`). |
| `ira-stt` | Synchronous `SttEngine`; `GrokStt` uploads mono PCM16 WAV; `NullStt` returns no transcript. |
| `ira-tts` | Synchronous `TtsEngine` returns mono PCM with a sample rate; `NullTts` generates a tone (the daemon's `speak` fallback is a beep). |

`GrokStt` and the daemon's `BlockingLlm` bridge async HTTP with
`tokio::runtime::Handle::block_on` — call them from blocking threads only.
Voice LLM requests contain the current user input and the voice system prompt
(`voice_system_prompt` setting) rather than persistent chat history.

## ira-daemon wiring

On start (`crates/ira-daemon/src/main.rs`):

1. Connect + migrate PostgreSQL, apply secrets to env, import the TOML file
   config once if settings are still default.
2. Load the `Snapshot`, build `EngineConfig` from voice settings
   (`audio_source`, `audio_sink`, barge-in, VAD hangover).
3. Load the wake model (optional), build STT (`grok` kind with API key, else
   `NullStt`), build the `BlockingLlm` (or `NullLlm` when the key is missing —
   the daemon warns and continues), `NullTts`.
4. `spawn_engine` starts the session thread; `ensure_voice` creates the
   single live `voice` conversation.
5. The `ira-events` thread consumes `SessionEvent`s: transcripts and replies
   are persisted to the voice conversation (`persist_voice`, fire-and-forget).
6. Binds the IPC socket and serves requests until shutdown.

`ira-ctl listen` is the intended hotkey target; the daemon prints the hint on
startup.
