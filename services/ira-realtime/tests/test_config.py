import pytest
from pydantic import ValidationError

from ira_realtime.config import Settings


def test_allowed_origins_are_split() -> None:
    settings = Settings(allowed_origins="http://localhost:5179, https://ira.test")
    assert settings.allowed_origins == ["http://localhost:5179", "https://ira.test"]


def test_default_audio_format() -> None:
    settings = Settings(_env_file=None)
    assert settings.sample_rate == 16000
    assert settings.channels == 1
    assert settings.mode == "echo"
    assert settings.ira_url == "http://127.0.0.1:8787"
    assert settings.tts == "pocket"
    assert settings.tts_language == "spanish"
    assert settings.tts_voice == "lola"
    assert settings.tts_quantize is True


def test_empty_tts_temp_is_none() -> None:
    settings = Settings(_env_file=None, tts_temp="  ")
    assert settings.tts_temp is None


def test_pocket_tts_requires_voice() -> None:
    with pytest.raises(ValidationError):
        Settings(_env_file=None, tts="pocket", tts_voice="")


def test_ira_url_is_stripped() -> None:
    settings = Settings(_env_file=None, mode="ira", ira_url="http://127.0.0.1:8787/")
    assert settings.ira_url == "http://127.0.0.1:8787"


def test_empty_conversation_id_is_none() -> None:
    settings = Settings(_env_file=None, conversation_id="  ")
    assert settings.conversation_id is None


def test_ira_mode_requires_url() -> None:
    with pytest.raises(ValidationError):
        Settings(_env_file=None, mode="ira", ira_url="   ")
