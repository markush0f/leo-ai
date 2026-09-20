from pipecat.frames.frames import (
    Frame,
    LLMFullResponseEndFrame,
    LLMFullResponseStartFrame,
    LLMTextFrame,
    TranscriptionFrame,
)
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor

from leo_realtime.agent import LeoAgentProcessor
from leo_realtime.leo_http import LeoHttpError


class FakeLeo:
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


class BrokenLeo(FakeLeo):
    async def chat(self, text: str) -> str:
        self.chats.append(text)
        raise LeoHttpError("sin postgres")


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
    leo = FakeLeo("hola desde leo")
    agent = LeoAgentProcessor(leo)
    sink = Sink()
    agent.link(sink)

    await agent.process_frame(_transcript("  hola  "), FrameDirection.DOWNSTREAM)

    assert leo.chats == ["hola"]
    kinds = [type(frame) for frame in sink.frames]
    assert kinds == [
        TranscriptionFrame,
        LLMFullResponseStartFrame,
        LLMTextFrame,
        LLMFullResponseEndFrame,
    ]
    assert sink.frames[2].text == "hola desde leo"


async def test_agent_skips_blank_transcript() -> None:
    leo = FakeLeo()
    agent = LeoAgentProcessor(leo)
    sink = Sink()
    agent.link(sink)

    await agent.process_frame(_transcript("   "), FrameDirection.DOWNSTREAM)

    assert leo.chats == []
    assert [type(frame) for frame in sink.frames] == [TranscriptionFrame]


async def test_agent_error_does_not_emit_llm_text() -> None:
    leo = BrokenLeo()
    agent = LeoAgentProcessor(leo)
    sink = Sink()
    agent.link(sink)

    await agent.process_frame(_transcript("hola"), FrameDirection.DOWNSTREAM)

    assert leo.chats == ["hola"]
    assert not any(isinstance(frame, LLMTextFrame) for frame in sink.frames)
