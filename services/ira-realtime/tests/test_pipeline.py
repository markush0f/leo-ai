import pytest
from pipecat.processors.frame_processor import FrameProcessor
from pipecat.transports.base_transport import BaseTransport

from ira_realtime.agent import IraAgentProcessor
from ira_realtime.config import Settings
from ira_realtime.pipeline import AudioEchoProcessor, create_processors


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


class FakeIra:
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


def test_ira_pipeline_inserts_agent() -> None:
    processors = create_processors(
        DummyTransport(),
        Settings(_env_file=None, mode="ira", tts="none"),
        client=FakeIra(),
    )
    assert [type(processor) for processor in processors] == [
        DummyProcessor,
        IraAgentProcessor,
        DummyProcessor,
    ]


def test_ira_pipeline_inserts_tts(monkeypatch: pytest.MonkeyPatch) -> None:
    tts = DummyProcessor("tts")
    monkeypatch.setattr("ira_realtime.pipeline.create_tts", lambda _settings: tts)
    processors = create_processors(
        DummyTransport(),
        Settings(_env_file=None, mode="ira", tts="pocket"),
        client=FakeIra(),
    )
    assert processors[-2] is tts


def test_ira_pipeline_requires_client() -> None:
    with pytest.raises(ValueError, match="Ira HTTP client"):
        create_processors(DummyTransport(), Settings(_env_file=None, mode="ira"))
