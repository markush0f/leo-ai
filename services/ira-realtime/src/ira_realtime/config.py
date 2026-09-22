"""Environment-backed service configuration."""

from functools import lru_cache
from typing import Literal

from pydantic import Field, field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Runtime settings loaded from `IRA_REALTIME_*` variables."""

    model_config = SettingsConfigDict(
        env_file=".env",
        env_prefix="IRA_REALTIME_",
        extra="ignore",
    )

    host: str = "0.0.0.0"
    port: int = Field(default=8765, ge=1, le=65535)
    mode: Literal["echo", "ira"] = "echo"
    sample_rate: int = Field(default=16000, ge=8000, le=48000)
    channels: int = Field(default=1, ge=1, le=2)
    log_level: str = "INFO"
    allowed_origins: list[str] = Field(default_factory=list)
    ira_url: str = "http://127.0.0.1:8787"
    conversation_id: str | None = None
    ira_timeout_secs: float = Field(default=120.0, gt=0)
    tts: Literal["none", "pocket"] = "pocket"
    tts_language: str = "spanish"
    tts_voice: str = "lola"
    tts_quantize: bool = True
    tts_temp: float | None = Field(default=None, ge=0)

    @field_validator("allowed_origins", mode="before")
    @classmethod
    def split_origins(cls, value: object) -> object:
        if isinstance(value, str):
            return [origin.strip() for origin in value.split(",") if origin.strip()]
        return value

    @field_validator("conversation_id", "tts_temp", mode="before")
    @classmethod
    def empty_optional(cls, value: object) -> object:
        if isinstance(value, str) and not value.strip():
            return None
        return value

    @field_validator("tts_language", "tts_voice", mode="before")
    @classmethod
    def strip_tts_fields(cls, value: object) -> object:
        if isinstance(value, str):
            return value.strip()
        return value

    @model_validator(mode="after")
    def normalize_ira_url(self) -> "Settings":
        self.ira_url = self.ira_url.strip().rstrip("/")
        if self.mode == "ira" and not self.ira_url:
            raise ValueError("IRA_REALTIME_IRA_URL is required when mode is ira")
        if self.tts == "pocket" and (not self.tts_language or not self.tts_voice):
            raise ValueError("IRA_REALTIME_TTS_LANGUAGE and IRA_REALTIME_TTS_VOICE are required")
        return self


@lru_cache
def get_settings() -> Settings:
    return Settings()
