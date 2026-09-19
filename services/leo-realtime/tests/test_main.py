from fastapi.testclient import TestClient

from leo_realtime.main import app


def test_healthz() -> None:
    response = TestClient(app).get("/healthz")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_audio_page() -> None:
    response = TestClient(app).get("/test")
    assert response.status_code == 200
    assert "Iniciar micrófono" in response.text
    assert "/ws/audio" in response.text
