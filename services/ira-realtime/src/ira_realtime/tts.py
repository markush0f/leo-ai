"""Text-to-speech for non-echo pipelines (Kyutai Pocket TTS)."""

from __future__ import annotations

import asyncio
import threading
from collections.abc import AsyncGenerator, AsyncIterator
from typing import Any

from loguru import logger
from pipecat.frames.frames import ErrorFrame, Frame
from pipecat.processors.frame_processor import FrameProcessor
from pipecat.services.settings import TTSSettings
from pipecat.services.tts_service import TTSService
from pipecat.transcriptions.language import Language
from pipecat.utils.tracing.service_decorators import traced_tts

from .config import Settings

_POCKET_FROM_BCP47 = {
    "de": "german",
    "en": "english",
    "es": "spanish",
    "fr": "french_24l",
    "it": "italian",
    "pt": "portuguese",
}

_models: dict[tuple[Any, ...], Any] = {}
_voices: dict[tuple[Any, ...], Any] = {}
_model_lock = threading.Lock()


def pocket_language(value: str) -> str:
    """Map BCP-47 tags to pocket-tts language names; pass other names through."""
    text = value.strip()
    if not text:
        raise ValueError("IRA_REALTIME_TTS_LANGUAGE is empty")
    try:
        language = Language(text)
    except ValueError:
        return text
    base = str(language).split("-", 1)[0].lower()
    return _POCKET_FROM_BCP47.get(base, text)


def reset_pocket_models() -> None:
    with _model_lock:
        _models.clear()
        _voices.clear()


def _model_key(settings: Settings) -> tuple[Any, ...]:
    return (pocket_language(settings.tts_language), settings.tts_quantize, settings.tts_temp)


def load_pocket_model(settings: Settings) -> Any:
    """Load a pocket-tts model once per (language, quantize, temp) in this process."""
    from pocket_tts import TTSModel

    key = _model_key(settings)
    with _model_lock:
        model = _models.get(key)
        if model is None:
            language, quantize, temp = key
            kwargs: dict[str, Any] = {"language": language, "quantize": quantize}
            if temp is not None:
                kwargs["temp"] = temp
            logger.info("Loading Pocket TTS model language={}", language)
            model = TTSModel.load_model(**kwargs)
            _models[key] = model
        return model


def load_pocket_voice(settings: Settings, model: Any) -> Any:
    """Keep derived voice state in memory; cloning from audio is slow."""
    key = (*_model_key(settings), settings.tts_voice)
    with _model_lock:
        state = _voices.get(key)
        if state is None:
            logger.info("Deriving Pocket TTS voice={}", settings.tts_voice)
            state = model.get_state_for_audio_prompt(settings.tts_voice)
            _voices[key] = state
        return state


class PocketTtsService(TTSService):
    """CPU Pocket TTS with a process-wide model cache (one processor per session)."""

    def __init__(self, settings: Settings, model: Any | None = None) -> None:
        language = pocket_language(settings.tts_language)
        super().__init__(
            push_start_frame=True,
            push_stop_frames=True,
            sample_rate=settings.sample_rate,
            settings=TTSSettings(model=None, voice=settings.tts_voice, language=language),
            name="pocket-tts",
        )
        self._model = model if model is not None else load_pocket_model(settings)
        self._voice_state = load_pocket_voice(settings, self._model)

    def can_generate_metrics(self) -> bool:
        return True

    @traced_tts
    async def run_tts(self, text: str, context_id: str) -> AsyncGenerator[Frame, None]:
        def sync_next(iterator: Any) -> Any:
            try:
                return next(iterator)
            except StopIteration:
                return None

        async def audio_iterator() -> AsyncIterator[bytes]:
            import torch

            stream = self._model.generate_audio_stream(self._voice_state, text, copy_state=True)
            while True:
                chunk = await asyncio.to_thread(sync_next, stream)
                if chunk is None:
                    return
                yield (chunk.clamp(-1.0, 1.0) * 32767).to(torch.int16).cpu().numpy().tobytes()

        try:
            await self.start_tts_usage_metrics(text)
            async for frame in self._stream_audio_frames_from_iterator(
                audio_iterator(),
                in_sample_rate=self._model.sample_rate,
                context_id=context_id,
            ):
                await self.stop_ttfb_metrics()
                yield frame
        except Exception as exc:
            logger.error("Pocket TTS failed: {}", exc)
            yield ErrorFrame(error=f"Pocket TTS: {exc}")
        finally:
            await self.stop_ttfb_metrics()


def create_tts(settings: Settings) -> FrameProcessor | None:
    """Return a TTS processor when one is configured; otherwise None.

    Without TTS, ira mode still sends the agent reply as a JSON WebSocket text
    frame.
    """
    if settings.tts == "none":
        return None
    if settings.tts != "pocket":
        raise ValueError(f"unsupported TTS backend: {settings.tts}")
    try:
        import pocket_tts  # noqa: F401
    except ModuleNotFoundError as exc:
        raise RuntimeError(
            'Pocket TTS no está instalado. En ira-realtime: pip install -e ".[tts]" '
            "--extra-index-url https://download.pytorch.org/whl/cpu"
        ) from exc
    return PocketTtsService(settings)
