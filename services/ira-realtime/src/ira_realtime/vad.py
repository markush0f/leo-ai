"""Voice activity detection extension point for non-echo pipelines."""

from pipecat.audio.vad.vad_analyzer import VADAnalyzer

from .config import Settings


def create_vad(_settings: Settings) -> VADAnalyzer | None:
    """Return no VAD analyzer because echo mode must preserve every sample."""
    return None
