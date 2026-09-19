# Leo Realtime

Prototype Pipecat service for bidirectional audio. Current `echo` mode proves
the complete WebSocket transport and Pipecat pipeline without API keys:

```text
WebSocket PCM16 -> Pipecat input -> echo processor -> Pipecat output -> WebSocket PCM16
```

## Protocol

- Endpoint: `ws://127.0.0.1:8765/ws/audio`
- Payload: binary little-endian signed PCM16
- Audio: mono, 16 kHz by default
- Packet recommendation: 640 bytes (20 ms)
- Output: one binary PCM16 message for every accepted input message

Text messages and odd-sized PCM payloads are ignored. WebSocket mode is for
local prototyping; use WebRTC before exposing this service in production.

## Run locally

```sh
cp .env.example .env
python -m venv .venv
. .venv/bin/activate
pip install -e '.[dev]'
leo-realtime
```

Send a generated one-second tone through the endpoint and save the returned
audio as `echo.wav`:

```sh
leo-realtime-client --output echo.wav
```

## Run with Docker

From repository root:

```sh
docker compose up --build leo-realtime
```

Health and metadata are available from `GET /healthz` and `GET /`.
Open `http://127.0.0.1:8765/test` for a browser microphone and playback test.
