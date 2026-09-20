"""Pipecat pipeline construction and lifecycle."""

from pipecat.frames.frames import Frame, InputAudioRawFrame, OutputAudioRawFrame
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.worker import PipelineParams, PipelineWorker
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.transports.base_transport import BaseTransport
from pipecat.workers.runner import WorkerRunner

from .agent import LeoAgentProcessor
from .config import Settings
from .leo_http import LeoAgent, LeoHttpClient, resolve_conversation_id
from .stt import create_stt
from .tts import create_tts


class AudioEchoProcessor(FrameProcessor):
    """Convert transport input audio into transport output audio unchanged."""

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if isinstance(frame, InputAudioRawFrame) and direction is FrameDirection.DOWNSTREAM:
            await self.push_frame(
                OutputAudioRawFrame(
                    audio=frame.audio,
                    sample_rate=frame.sample_rate,
                    num_channels=frame.num_channels,
                )
            )
            return
        await self.push_frame(frame, direction)


def create_processors(
    transport: BaseTransport,
    settings: Settings,
    client: LeoAgent | None = None,
) -> list[FrameProcessor]:
    incoming = transport.input()
    outgoing = transport.output()
    if settings.mode == "echo":
        return [incoming, AudioEchoProcessor(), outgoing]
    if client is None:
        raise ValueError("leo mode requires a Leo HTTP client")
    processors: list[FrameProcessor] = [incoming]
    stt = create_stt(settings)
    if stt is not None:
        processors.append(stt)
    processors.append(LeoAgentProcessor(client))
    tts = create_tts(settings)
    if tts is not None:
        processors.append(tts)
    processors.append(outgoing)
    return processors


def create_worker(
    transport: BaseTransport,
    settings: Settings,
    client: LeoAgent | None = None,
) -> PipelineWorker:
    pipeline = Pipeline(create_processors(transport, settings, client))
    return PipelineWorker(
        pipeline,
        name="leo-realtime",
        enable_rtvi=False,
        idle_timeout_secs=None,
        params=PipelineParams(
            audio_in_sample_rate=settings.sample_rate,
            audio_out_sample_rate=settings.sample_rate,
            enable_metrics=True,
        ),
    )


async def run_pipeline(
    transport: BaseTransport,
    settings: Settings,
    conversation_id: str | None = None,
) -> None:
    client: LeoHttpClient | None = None
    if settings.mode == "leo":
        client = LeoHttpClient(
            settings.leo_url,
            timeout=settings.leo_timeout_secs,
            conversation_id=resolve_conversation_id(conversation_id, settings.conversation_id),
        )
    runner = WorkerRunner(handle_sigint=False, handle_sigterm=False)
    worker = create_worker(transport, settings, client)

    @transport.event_handler("on_client_disconnected")
    async def on_client_disconnected(_transport: BaseTransport, _client: object) -> None:
        await runner.cancel(reason="WebSocket client disconnected")

    try:
        await runner.add_workers(worker)
        await runner.run()
    finally:
        if client is not None:
            await client.aclose()
