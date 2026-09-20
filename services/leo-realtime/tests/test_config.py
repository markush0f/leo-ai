import pytest
from pydantic import ValidationError

from leo_realtime.config import Settings


def test_allowed_origins_are_split() -> None:
    settings = Settings(allowed_origins="http://localhost:5179, https://leo.test")
    assert settings.allowed_origins == ["http://localhost:5179", "https://leo.test"]


def test_default_audio_format() -> None:
    settings = Settings(_env_file=None)
    assert settings.sample_rate == 16000
    assert settings.channels == 1
    assert settings.mode == "echo"
    assert settings.leo_url == "http://127.0.0.1:8787"


def test_leo_url_is_stripped() -> None:
    settings = Settings(_env_file=None, mode="leo", leo_url="http://127.0.0.1:8787/")
    assert settings.leo_url == "http://127.0.0.1:8787"


def test_empty_conversation_id_is_none() -> None:
    settings = Settings(_env_file=None, conversation_id="  ")
    assert settings.conversation_id is None


def test_leo_mode_requires_url() -> None:
    with pytest.raises(ValidationError):
        Settings(_env_file=None, mode="leo", leo_url="   ")
