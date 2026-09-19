"""Environment-backed service configuration."""

from functools import lru_cache
from typing import Literal

from pydantic import Field, field_validator
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
    mode: Literal["echo"] = "echo"
    sample_rate: int = Field(default=16000, ge=8000, le=48000)
    channels: int = Field(default=1, ge=1, le=2)
    log_level: str = "INFO"
    allowed_origins: list[str] = Field(default_factory=list)

    @field_validator("allowed_origins", mode="before")
    @classmethod
    def split_origins(cls, value: object) -> object:
        if isinstance(value, str):
            return [origin.strip() for origin in value.split(",") if origin.strip()]
        return value


@lru_cache
def get_settings() -> Settings:
    return Settings()
