import json

import httpx
import pytest

from ira_realtime.ira_http import IraHttpClient, IraHttpError, resolve_conversation_id


def _client(handler) -> IraHttpClient:
    transport = httpx.MockTransport(handler)
    return IraHttpClient(
        "http://ira.test",
        timeout=5,
        http=httpx.AsyncClient(transport=transport),
    )


def test_resolve_conversation_id_prefers_query() -> None:
    assert resolve_conversation_id("  abc  ", "env-id") == "abc"
    assert resolve_conversation_id("", "env-id") == "env-id"
    assert resolve_conversation_id(None, None) is None


@pytest.mark.asyncio
async def test_creates_conversation_then_chats() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        if request.url.path == "/api/chats":
            return httpx.Response(
                200,
                json={"id": "11111111-1111-1111-1111-111111111111", "title": None},
            )
        if request.url.path.endswith("/messages"):
            body = json.loads(request.content)
            return httpx.Response(200, json={"text": f"ira:{body['text']}"})
        return httpx.Response(404)

    client = _client(handler)
    assert await client.chat("hola") == "ira:hola"
    assert [request.url.path for request in requests] == [
        "/api/chats",
        "/api/chats/11111111-1111-1111-1111-111111111111/messages",
    ]
    assert json.loads(requests[1].content) == {"text": "hola"}
    await client.aclose()


@pytest.mark.asyncio
async def test_reuses_conversation_id() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/api/chats/abc/messages"
        return httpx.Response(200, json={"text": "ok"})

    client = IraHttpClient(
        "http://ira.test",
        timeout=5,
        conversation_id="abc",
        http=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
    )
    assert await client.chat("ping") == "ok"
    await client.aclose()


@pytest.mark.asyncio
async def test_surfaces_server_error() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        return httpx.Response(400, json={"error": "mensaje vacío"})

    client = IraHttpClient(
        "http://ira.test",
        timeout=5,
        conversation_id="abc",
        http=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
    )
    with pytest.raises(IraHttpError, match="mensaje vacío"):
        await client.chat("   ")
    await client.aclose()
