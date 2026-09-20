"""Environment-backed service configuration."""

from functools import lru_cache
from typing import Literal

from pydantic import Field, field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Runtime settings loaded from `LEO_REALTIME_*` variables."""

    model_config = SettingsConfigDict(
        env_file=".env",
        env_prefix="LEO_REALTIME_",
        extra="ignore",
    )

    host: str = "0.0.0.0"
    port: int = Field(default=8765, ge=1, le=65535)
    mode: Literal["echo", "leo"] = "echo"
    sample_rate: int = Field(default=16000, ge=8000, le=48000)
    channels: int = Field(default=1, ge=1, le=2)
    log_level: str = "INFO"
    allowed_origins: list[str] = Field(default_factory=list)
    leo_url: str = "http://127.0.0.1:8787"
    conversation_id: str | None = None
    leo_timeout_secs: float = Field(default=120.0, gt=0)

    @field_validator("allowed_origins", mode="before")
    @classmethod
    def split_origins(cls, value: object) -> object:
        if isinstance(value, str):
            return [origin.strip() for origin in value.split(",") if origin.strip()]
        return value

    @field_validator("conversation_id", mode="before")
    @classmethod
    def empty_conversation_id(cls, value: object) -> object:
        if isinstance(value, str) and not value.strip():
            return None
        return value

    @model_validator(mode="after")
    def normalize_leo_url(self) -> "Settings":
        self.leo_url = self.leo_url.strip().rstrip("/")
        if self.mode == "leo" and not self.leo_url:
            raise ValueError("LEO_REALTIME_LEO_URL is required when mode is leo")
        return self


@lru_cache
def get_settings() -> Settings:
    return Settings()
