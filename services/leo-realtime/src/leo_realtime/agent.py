"""Pipecat processor that sends transcripts to the Leo HTTP agent."""

from __future__ import annotations

import asyncio

from pipecat.frames.frames import (
    Frame,
    LLMFullResponseEndFrame,
    LLMFullResponseStartFrame,
    LLMTextFrame,
    TranscriptionFrame,
)
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor

from .leo_http import LeoAgent, LeoHttpError


class LeoAgentProcessor(FrameProcessor):
    """Turn a final transcript into one Leo HTTP reply for TTS."""

    def __init__(self, client: LeoAgent) -> None:
        super().__init__(name="leo-agent")
        self._client = client
        self._lock = asyncio.Lock()

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if direction is not FrameDirection.DOWNSTREAM:
            await self.push_frame(frame, direction)
            return

        if isinstance(frame, TranscriptionFrame):
            await self.push_frame(frame, direction)
            text = frame.text.strip()
            if not text:
                return
            async with self._lock:
                try:
                    reply = await self._client.chat(text)
                except LeoHttpError as exc:
                    await self.push_error(error_msg=str(exc), exception=exc)
                    return
            reply = reply.strip()
            if not reply:
                return
            await self.push_frame(LLMFullResponseStartFrame())
            await self.push_frame(LLMTextFrame(text=reply))
            await self.push_frame(LLMFullResponseEndFrame())
            return

        await self.push_frame(frame, direction)
