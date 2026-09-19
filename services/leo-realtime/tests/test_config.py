from leo_realtime.config import Settings


def test_allowed_origins_are_split() -> None:
    settings = Settings(allowed_origins="http://localhost:5179, https://leo.test")
    assert settings.allowed_origins == ["http://localhost:5179", "https://leo.test"]


def test_default_audio_format() -> None:
    settings = Settings(_env_file=None)
    assert settings.sample_rate == 16000
    assert settings.channels == 1
