"""Raw PCM serializer and FastAPI Pipecat transport."""

from fastapi import WebSocket
from pipecat.frames.frames import Frame, InputAudioRawFrame, OutputAudioRawFrame
from pipecat.serializers.base_serializer import FrameSerializer
from pipecat.transports.websocket.fastapi import (
    FastAPIWebsocketParams,
    FastAPIWebsocketTransport,
)

from .config import Settings


class RawPcmSerializer(FrameSerializer):
    """Map each binary WebSocket message to one PCM16 audio frame."""

    def __init__(self, sample_rate: int, channels: int) -> None:
        super().__init__()
        self.sample_rate = sample_rate
        self.channels = channels

    async def serialize(self, frame: Frame) -> bytes | None:
        if isinstance(frame, OutputAudioRawFrame):
            return frame.audio
        return None

    async def deserialize(self, data: str | bytes) -> Frame | None:
        if not isinstance(data, bytes) or not data or len(data) % 2:
            return None
        return InputAudioRawFrame(
            audio=data,
            sample_rate=self.sample_rate,
            num_channels=self.channels,
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
