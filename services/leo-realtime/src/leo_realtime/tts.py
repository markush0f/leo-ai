"""Text-to-speech extension point for non-echo pipelines."""

from pipecat.processors.frame_processor import FrameProcessor

from .config import Settings


def create_tts(_settings: Settings) -> FrameProcessor | None:
    """Return no TTS processor while the service runs in transport-test mode."""
    return None
