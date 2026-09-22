# Ira Realtime

Pipecat service for bidirectional audio. `echo` mode proves the WebSocket
transport without API keys. `ira` mode sends transcripts to the Ira agent over
HTTP (`ira-server`) and returns the reply as JSON text (and TTS audio when a
TTS processor is configured).

```text
echo:
  WebSocket PCM16 -> Pipecat input -> echo processor -> Pipecat output -> WebSocket PCM16

ira:
  WebSocket PCM16 or text -> STT (optional) -> POST ira-server /api/chats/{id}/messages
    -> TTS (optional) -> WebSocket PCM16 and/or JSON text
```

## Protocol

- Endpoint: `ws://127.0.0.1:8765/ws/audio`
- Binary payload: little-endian signed PCM16, mono, 16 kHz by default
- Packet recommendation: 640 bytes (20 ms)
- Text payload: plain UTF-8 or `{"text": "..."}` (treated as a user transcript)
- Odd-sized PCM payloads are ignored
- In `echo` mode, each accepted PCM message is echoed as PCM
- In `ira` mode, each Ira reply is sent as `{"type":"assistant","text":"..."}`
  and, with Pocket TTS, as PCM16 audio at the configured sample rate

WebSocket mode is for local prototyping; use WebRTC before exposing this
service in production.

## Connect to Ira

1. Start `ira-server` (default `http://127.0.0.1:8787`).
2. Set `IRA_REALTIME_MODE=ira` and `IRA_REALTIME_IRA_URL` to that origin.
3. Restart `ira-realtime`. Each WebSocket session creates a conversation unless
   `IRA_REALTIME_CONVERSATION_ID` is set, or the client passes
   `?conversation_id=` on `ws://127.0.0.1:8765/ws/audio`.

The agent call is a blocking HTTP turn: Pipecat waits for `{ "text": "..." }`
before continuing. Tools, catalog, and history stay in Ira; this service does
not call the LLM provider itself.

STT still returns `None`; send text on the WebSocket (browser Web Speech or
`{"text": "..."}`). TTS defaults to Kyutai [Pocket TTS](https://github.com/kyutai-labs/pocket-tts)
on CPU (`IRA_REALTIME_TTS=pocket`): Spanish weights and the `lola` voice.
Set `IRA_REALTIME_TTS=none` to keep JSON text only.

Pocket TTS speaks at 24 kHz; the pipeline resamples to the WebSocket rate
(16 kHz). The first synthesis downloads model weights from Hugging Face and
can take a minute. Docker stores that cache in the `ira-hf` volume.

## Run locally

```sh
cp .env.example .env
python -m venv .venv
. .venv/bin/activate
pip install -e '.[dev]'
# CPU PyTorch (Linux). Skip this if you only run echo mode or TTS=none.
pip install -e '.[tts]' --extra-index-url https://download.pytorch.org/whl/cpu
ira-realtime
```

Send a generated one-second tone through the endpoint and save the returned
audio as `echo.wav`:

```sh
ira-realtime-client --output echo.wav
```

## Run with Docker

From repository root:

```sh
./scripts/start-ira-realtime.sh
```

Set `IRA_REALTIME_MODE=echo` before running it when `ira-server` is not active.

Health and metadata are available from `GET /healthz` and `GET /`.
Open `http://127.0.0.1:8765/test` for a browser microphone and playback test.
