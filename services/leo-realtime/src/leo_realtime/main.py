"""FastAPI entry point for Leo realtime audio."""

from pathlib import Path

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse

from .config import get_settings
from .pipeline import run_pipeline
from .transport import create_transport

settings = get_settings()
app = FastAPI(title="Leo Realtime", version="0.1.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)
test_page = Path(__file__).with_name("static") / "index.html"


@app.get("/")
async def root() -> dict[str, object]:
    payload: dict[str, object] = {
        "service": "leo-realtime",
        "mode": settings.mode,
        "audio_endpoint": "/ws/audio",
        "test_page": "/test",
        "format": f"pcm_s16le/{settings.sample_rate}/{settings.channels}",
        "tts": settings.tts,
    }
    if settings.mode == "leo":
        payload["leo_url"] = settings.leo_url
    if settings.tts == "pocket":
        payload["tts_language"] = settings.tts_language
        payload["tts_voice"] = settings.tts_voice
    return payload


@app.get("/test", response_class=FileResponse)
async def test_audio() -> FileResponse:
    return FileResponse(test_page)


@app.get("/healthz")
async def healthz() -> dict[str, str]:
    return {"status": "ok"}


@app.websocket("/ws/audio")
async def audio_endpoint(websocket: WebSocket) -> None:
    conversation_id = websocket.query_params.get("conversation_id")
    await websocket.accept()
    try:
        transport = create_transport(websocket, settings)
        await run_pipeline(transport, settings, conversation_id=conversation_id)
    except WebSocketDisconnect:
        pass
    except ValueError:
        await websocket.close(code=1008)


def run() -> None:
    import uvicorn

    uvicorn.run(
        "leo_realtime.main:app",
        host=settings.host,
        port=settings.port,
        log_level=settings.log_level.lower(),
    )


if __name__ == "__main__":
    run()
