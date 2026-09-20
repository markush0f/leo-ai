"""HTTP client for the Leo agent on leo-server."""

from __future__ import annotations

from typing import Protocol

import httpx


class LeoAgent(Protocol):
    """Turn-based text agent used by the Pipecat processor."""

    async def ensure_conversation(self) -> str: ...

    async def chat(self, text: str) -> str: ...

    async def aclose(self) -> None: ...


def resolve_conversation_id(*candidates: str | None) -> str | None:
    """First non-empty conversation id wins (WebSocket query, then settings)."""
    for value in candidates:
        if value and value.strip():
            return value.strip()
    return None


class LeoHttpError(Exception):
    """leo-server returned an error or an unexpected payload."""

    def __init__(self, message: str, status_code: int | None = None) -> None:
        super().__init__(message)
        self.status_code = status_code


class LeoHttpClient:
    """POST /api/chats and /api/chats/{id}/messages against leo-server."""

    def __init__(
        self,
        base_url: str,
        timeout: float,
        conversation_id: str | None = None,
        http: httpx.AsyncClient | None = None,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._conversation_id = conversation_id
        self._owns_http = http is None
        self._http = http or httpx.AsyncClient(timeout=timeout)

    async def aclose(self) -> None:
        if self._owns_http:
            await self._http.aclose()

    async def ensure_conversation(self) -> str:
        if self._conversation_id:
            return self._conversation_id
        try:
            response = await self._http.post(f"{self._base_url}/api/chats")
        except httpx.HTTPError as exc:
            raise LeoHttpError(f"no se pudo crear el chat: {exc}") from exc
        payload = _json_object(response)
        conversation_id = payload.get("id")
        if not isinstance(conversation_id, str) or not conversation_id:
            raise LeoHttpError("leo-server no devolvió id de conversación", response.status_code)
        self._conversation_id = conversation_id
        return conversation_id

    async def chat(self, text: str) -> str:
        conversation_id = await self.ensure_conversation()
        try:
            response = await self._http.post(
                f"{self._base_url}/api/chats/{conversation_id}/messages",
                json={"text": text},
            )
        except httpx.HTTPError as exc:
            raise LeoHttpError(f"no se pudo hablar con Leo: {exc}") from exc
        payload = _json_object(response)
        reply = payload.get("text")
        if not isinstance(reply, str):
            raise LeoHttpError("leo-server no devolvió texto", response.status_code)
        return reply


def _json_object(response: httpx.Response) -> dict[str, object]:
    try:
        payload = response.json()
    except ValueError as exc:
        raise LeoHttpError(
            f"http {response.status_code}: respuesta no JSON",
            response.status_code,
        ) from exc
    if not isinstance(payload, dict):
        raise LeoHttpError(f"http {response.status_code}: JSON inesperado", response.status_code)
    if not response.is_success:
        error = payload.get("error")
        detail = error if isinstance(error, str) and error else response.text
        raise LeoHttpError(detail, response.status_code)
    return payload
