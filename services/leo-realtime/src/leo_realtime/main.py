"""FastAPI entry point for Leo realtime audio."""

from pathlib import Path

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.responses import FileResponse

from .config import get_settings
from .pipeline import run_pipeline
from .transport import create_transport

settings = get_settings()
app = FastAPI(title="Leo Realtime", version="0.1.0")
test_page = Path(__file__).with_name("static") / "index.html"


@app.get("/")
async def root() -> dict[str, object]:
    return {
        "service": "leo-realtime",
        "mode": settings.mode,
        "audio_endpoint": "/ws/audio",
        "test_page": "/test",
        "format": f"pcm_s16le/{settings.sample_rate}/{settings.channels}",
    }


@app.get("/test", response_class=FileResponse)
async def test_audio() -> FileResponse:
    return FileResponse(test_page)


@app.get("/healthz")
async def healthz() -> dict[str, str]:
    return {"status": "ok"}


@app.websocket("/ws/audio")
async def audio_endpoint(websocket: WebSocket) -> None:
    await websocket.accept()
    try:
        transport = create_transport(websocket, settings)
        await run_pipeline(transport, settings)
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
