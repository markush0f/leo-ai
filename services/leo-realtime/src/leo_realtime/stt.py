"""Speech-to-text extension point for non-echo pipelines."""

from pipecat.processors.frame_processor import FrameProcessor

from .config import Settings


def create_stt(_settings: Settings) -> FrameProcessor | None:
    """Return no STT processor while the service runs in transport-test mode."""
    return None
