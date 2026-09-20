"""Raw PCM serializer and FastAPI Pipecat transport."""

import json
from datetime import UTC, datetime

from fastapi import WebSocket
from pipecat.frames.frames import (
    Frame,
    InputAudioRawFrame,
    LLMTextFrame,
    OutputAudioRawFrame,
    TranscriptionFrame,
    TTSTextFrame,
)
from pipecat.serializers.base_serializer import FrameSerializer
from pipecat.transports.websocket.fastapi import (
    FastAPIWebsocketParams,
    FastAPIWebsocketTransport,
)

from .config import Settings


class RawPcmSerializer(FrameSerializer):
    """Map binary WebSocket messages to PCM16 and text messages to transcripts."""

    def __init__(self, sample_rate: int, channels: int) -> None:
        super().__init__()
        self.sample_rate = sample_rate
        self.channels = channels

    async def serialize(self, frame: Frame) -> str | bytes | None:
        if isinstance(frame, OutputAudioRawFrame):
            return frame.audio
        if isinstance(frame, (LLMTextFrame, TTSTextFrame)):
            text = frame.text.strip()
            if not text:
                return None
            return json.dumps({"type": "assistant", "text": text}, ensure_ascii=False)
        return None

    async def deserialize(self, data: str | bytes) -> Frame | None:
        if isinstance(data, str):
            return _transcription_from_text(data)
        if not isinstance(data, bytes) or not data or len(data) % 2:
            return None
        return InputAudioRawFrame(
            audio=data,
            sample_rate=self.sample_rate,
            num_channels=self.channels,
        )


def _transcription_from_text(data: str) -> TranscriptionFrame | None:
    text = data.strip()
    if not text:
        return None
    if text.startswith("{"):
        try:
            payload = json.loads(text)
        except json.JSONDecodeError:
            return None
        if not isinstance(payload, dict):
            return None
        raw = payload.get("text")
        text = raw.strip() if isinstance(raw, str) else ""
        if not text:
            return None
    return TranscriptionFrame(
        text=text,
        user_id="",
        timestamp=datetime.now(UTC).isoformat(),
        finalized=True,
    )


def create_transport(websocket: WebSocket, settings: Settings) -> FastAPIWebsocketTransport:
    serializer = RawPcmSerializer(settings.sample_rate, settings.channels)
    params = FastAPIWebsocketParams(
        audio_in_enabled=True,
        audio_out_enabled=True,
        audio_in_sample_rate=settings.sample_rate,
        audio_out_sample_rate=settings.sample_rate,
        audio_in_channels=settings.channels,
        audio_out_channels=settings.channels,
        audio_out_10ms_chunks=2,
        add_wav_header=False,
        serializer=serializer,
        allowed_origins=settings.allowed_origins,
    )
    return FastAPIWebsocketTransport(websocket=websocket, params=params)
