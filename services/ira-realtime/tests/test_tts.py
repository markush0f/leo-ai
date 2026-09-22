import sys
from types import SimpleNamespace
from unittest.mock import MagicMock

import pytest
from pipecat.processors.frame_processor import FrameProcessor

from ira_realtime.config import Settings
from ira_realtime.tts import PocketTtsService, create_tts, pocket_language, reset_pocket_models


@pytest.fixture(autouse=True)
def _clear_models() -> None:
    reset_pocket_models()
    yield
    reset_pocket_models()


def test_pocket_language_maps_bcp47() -> None:
    assert pocket_language("es") == "spanish"
    assert pocket_language("es-ES") == "spanish"
    assert pocket_language("en-US") == "english"
    assert pocket_language("spanish") == "spanish"
    assert pocket_language("spanish_24l") == "spanish_24l"


def test_create_tts_none() -> None:
    assert create_tts(Settings(_env_file=None, tts="none")) is None


def test_create_tts_missing_package(monkeypatch: pytest.MonkeyPatch) -> None:
    import builtins

    monkeypatch.delitem(sys.modules, "pocket_tts", raising=False)
    real_import = builtins.__import__

    def fake_import(name, *args, **kwargs):
        if name == "pocket_tts":
            raise ModuleNotFoundError("pocket_tts")
        return real_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, "__import__", fake_import)
    with pytest.raises(RuntimeError, match="Pocket TTS no está instalado"):
        create_tts(Settings(_env_file=None, tts="pocket"))


def test_create_tts_pocket_builds_service(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setitem(sys.modules, "pocket_tts", SimpleNamespace())
    model = SimpleNamespace(
        sample_rate=24000,
        get_state_for_audio_prompt=MagicMock(return_value={"voice": "lola"}),
    )
    load = MagicMock(return_value=model)
    monkeypatch.setattr("ira_realtime.tts.load_pocket_model", load)

    first = create_tts(Settings(_env_file=None, tts="pocket"))
    second = create_tts(Settings(_env_file=None, tts="pocket"))

    assert isinstance(first, PocketTtsService)
    assert isinstance(second, FrameProcessor)
    assert load.call_count == 2
    assert first._model is model
    assert second._model is model


def test_load_pocket_model_is_cached(monkeypatch: pytest.MonkeyPatch) -> None:
    from ira_realtime import tts as tts_mod

    fake_model = object()
    load_model = MagicMock(return_value=fake_model)
    fake_tts_model = SimpleNamespace(load_model=load_model)
    monkeypatch.setitem(sys.modules, "pocket_tts", SimpleNamespace(TTSModel=fake_tts_model))

    settings = Settings(_env_file=None, tts="pocket", tts_language="spanish")
    first = tts_mod.load_pocket_model(settings)
    second = tts_mod.load_pocket_model(settings)
    assert first is second is fake_model
    load_model.assert_called_once()
