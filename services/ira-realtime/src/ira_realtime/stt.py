"""Speech-to-text extension point for non-echo pipelines."""

from pipecat.processors.frame_processor import FrameProcessor

from .config import Settings


def create_stt(_settings: Settings) -> FrameProcessor | None:
    """Return an STT processor when one is configured; otherwise None.

    Without STT, ira mode still accepts WebSocket text as a transcript.
    """
    return None
