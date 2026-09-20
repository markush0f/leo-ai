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


def test_root_reports_echo_mode() -> None:
    response = TestClient(app).get("/")
    assert response.status_code == 200
    body = response.json()
    assert body["service"] == "leo-realtime"
    assert body["mode"] == "echo"
    assert "leo_url" not in body


def test_root_allows_browser_origins() -> None:
    response = TestClient(app).get("/", headers={"Origin": "http://127.0.0.1:5179"})
    assert response.status_code == 200
    assert response.headers.get("access-control-allow-origin") == "*"
