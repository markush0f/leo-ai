"""Pipecat pipeline construction and lifecycle."""

from pipecat.frames.frames import Frame, InputAudioRawFrame, OutputAudioRawFrame
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.worker import PipelineParams, PipelineWorker
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.transports.base_transport import BaseTransport
from pipecat.workers.runner import WorkerRunner

from .config import Settings


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


def create_worker(transport: BaseTransport, settings: Settings) -> PipelineWorker:
    pipeline = Pipeline([transport.input(), AudioEchoProcessor(), transport.output()])
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


async def run_pipeline(transport: BaseTransport, settings: Settings) -> None:
    runner = WorkerRunner(handle_sigint=False, handle_sigterm=False)
    worker = create_worker(transport, settings)

    @transport.event_handler("on_client_disconnected")
    async def on_client_disconnected(_transport: BaseTransport, _client: object) -> None:
        await runner.cancel(reason="WebSocket client disconnected")

    await runner.add_workers(worker)
    await runner.run()
