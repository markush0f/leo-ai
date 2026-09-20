import pytest
from pipecat.processors.frame_processor import FrameProcessor
from pipecat.transports.base_transport import BaseTransport

from leo_realtime.agent import LeoAgentProcessor
from leo_realtime.config import Settings
from leo_realtime.pipeline import AudioEchoProcessor, create_processors


class DummyProcessor(FrameProcessor):
    def __init__(self, name: str) -> None:
        super().__init__(enable_direct_mode=True, name=name)


class DummyTransport(BaseTransport):
    def __init__(self) -> None:
        super().__init__()
        self._in = DummyProcessor("in")
        self._out = DummyProcessor("out")

    def input(self) -> FrameProcessor:
        return self._in

    def output(self) -> FrameProcessor:
        return self._out


class FakeLeo:
    async def ensure_conversation(self) -> str:
        return "cid"

    async def chat(self, text: str) -> str:
        return text

    async def aclose(self) -> None:
        return None


def test_echo_pipeline_is_passthrough() -> None:
    processors = create_processors(DummyTransport(), Settings(_env_file=None, mode="echo"))
    assert [type(processor) for processor in processors] == [
        DummyProcessor,
        AudioEchoProcessor,
        DummyProcessor,
    ]


def test_leo_pipeline_inserts_agent() -> None:
    processors = create_processors(
        DummyTransport(),
        Settings(_env_file=None, mode="leo", tts="none"),
        client=FakeLeo(),
    )
    assert [type(processor) for processor in processors] == [
        DummyProcessor,
        LeoAgentProcessor,
        DummyProcessor,
    ]


def test_leo_pipeline_inserts_tts(monkeypatch: pytest.MonkeyPatch) -> None:
    tts = DummyProcessor("tts")
    monkeypatch.setattr("leo_realtime.pipeline.create_tts", lambda _settings: tts)
    processors = create_processors(
        DummyTransport(),
        Settings(_env_file=None, mode="leo", tts="pocket"),
        client=FakeLeo(),
    )
    assert processors[-2] is tts


def test_leo_pipeline_requires_client() -> None:
    with pytest.raises(ValueError, match="Leo HTTP client"):
        create_processors(DummyTransport(), Settings(_env_file=None, mode="leo"))
