from pipecat.frames.frames import (
    Frame,
    LLMFullResponseEndFrame,
    LLMFullResponseStartFrame,
    LLMTextFrame,
    TranscriptionFrame,
)
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor

from ira_realtime.agent import IraAgentProcessor
from ira_realtime.ira_http import IraHttpError


class FakeIra:
    def __init__(self, reply: str = "respuesta") -> None:
        self.reply = reply
        self.chats: list[str] = []

    async def ensure_conversation(self) -> str:
        return "cid"

    async def chat(self, text: str) -> str:
        self.chats.append(text)
        return self.reply

    async def aclose(self) -> None:
        return None


class BrokenIra(FakeIra):
    async def chat(self, text: str) -> str:
        self.chats.append(text)
        raise IraHttpError("sin postgres")


class Sink(FrameProcessor):
    def __init__(self) -> None:
        super().__init__(enable_direct_mode=True, name="sink")
        self.frames: list[object] = []

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        self.frames.append(frame)


def _transcript(text: str) -> TranscriptionFrame:
    return TranscriptionFrame(text=text, user_id="", timestamp="t", finalized=True)


async def test_agent_posts_transcript_and_emits_llm_frames() -> None:
    ira = FakeIra("hola desde ira")
    agent = IraAgentProcessor(ira)
    sink = Sink()
    agent.link(sink)

    await agent.process_frame(_transcript("  hola  "), FrameDirection.DOWNSTREAM)

    assert ira.chats == ["hola"]
    kinds = [type(frame) for frame in sink.frames]
    assert kinds == [
        TranscriptionFrame,
        LLMFullResponseStartFrame,
        LLMTextFrame,
        LLMFullResponseEndFrame,
    ]
    assert sink.frames[2].text == "hola desde ira"


async def test_agent_skips_blank_transcript() -> None:
    ira = FakeIra()
    agent = IraAgentProcessor(ira)
    sink = Sink()
    agent.link(sink)

    await agent.process_frame(_transcript("   "), FrameDirection.DOWNSTREAM)

    assert ira.chats == []
    assert [type(frame) for frame in sink.frames] == [TranscriptionFrame]


async def test_agent_error_does_not_emit_llm_text() -> None:
    ira = BrokenIra()
    agent = IraAgentProcessor(ira)
    sink = Sink()
    agent.link(sink)

    await agent.process_frame(_transcript("hola"), FrameDirection.DOWNSTREAM)

    assert ira.chats == ["hola"]
    assert not any(isinstance(frame, LLMTextFrame) for frame in sink.frames)
