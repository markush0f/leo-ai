import json
import threading
from types import SimpleNamespace
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import pytest

from leo_wren import cli


class FakeTable:
    column_names = ["conversation_count"]
    num_rows = 1

    def to_pylist(self):
        return [{"conversation_count": 3}]


class FakeToolkit:
    def dry_plan(self, sql):
        return f"planned: {sql}"

    def query(self, sql):
        return FakeTable()


def test_check_plans_and_executes_query(monkeypatch, capsys, tmp_path):
    monkeypatch.setattr(cli, "load_toolkit", lambda project, profile=None: FakeToolkit())

    assert cli.main(["--project", str(tmp_path), "check"]) == 0

    output = json.loads(capsys.readouterr().out)
    assert output["status"] == "ok"
    assert output["result"]["rows"] == [{"conversation_count": 3}]


def test_query_serializes_arrow_compatible_table(monkeypatch, capsys, tmp_path):
    monkeypatch.setattr(cli, "load_toolkit", lambda project, profile=None: FakeToolkit())

    assert cli.main(["--project", str(tmp_path), "query", "SELECT 1"]) == 0

    output = json.loads(capsys.readouterr().out)
    assert output["columns"] == ["conversation_count"]
    assert output["row_count"] == 1


def test_project_argument_defaults_to_service_directory():
    args = cli.create_parser().parse_args(["dry-plan", "SELECT 1"])

    assert args.project == cli.DEFAULT_PROJECT


def test_profile_can_be_selected_per_invocation(monkeypatch, tmp_path):
    selected = []
    monkeypatch.setattr(
        cli,
        "load_toolkit",
        lambda project, profile=None: selected.append((project, profile)) or FakeToolkit(),
    )

    assert cli.main(["--project", str(tmp_path), "--profile", "warehouse", "dry-plan", "SELECT 1"]) == 0
    assert selected == [(tmp_path, "warehouse")]


def test_discover_project_prefers_working_directory_over_installed_module(tmp_path):
    project = tmp_path / "app"
    project.mkdir()
    (project / "wren_project.yml").write_text("name: leo\n", encoding="utf-8")
    installed = tmp_path / "site-packages" / "leo_wren" / "cli.py"
    installed.parent.mkdir(parents=True)

    assert cli.discover_project(project, installed) == project


def test_ask_requires_agent_extra(monkeypatch, tmp_path):
    monkeypatch.setattr(cli, "load_toolkit", lambda project, profile=None: FakeToolkit())
    monkeypatch.setitem(__import__("sys").modules, "langchain.agents", None)
    args = SimpleNamespace(
        project=tmp_path,
        profile=None,
        command="ask",
        question="How many conversations?",
        model="openai:gpt-4o-mini",
    )

    with pytest.raises(RuntimeError, match="agent extra"):
        cli.run(args)


def _start(tmp_path, monkeypatch, refresh_toolkit=None):
    monkeypatch.setattr(cli, "load_toolkit", lambda project, profile=None: FakeToolkit())
    web = tmp_path / "web"
    web.mkdir()
    (web / "index.html").write_text("<!doctype html><title>Leo Wren</title>", encoding="utf-8")
    server = cli.make_server(
        tmp_path, FakeToolkit(), "127.0.0.1", 0, refresh_toolkit
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def test_page_lists_sql_and_returns_rows(monkeypatch, tmp_path):
    server = _start(tmp_path, monkeypatch)
    port = server.server_address[1]
    try:
        page = urlopen(f"http://127.0.0.1:{port}/").read().decode()
        assert "Leo Wren" in page

        request = Request(
            f"http://127.0.0.1:{port}/api/query",
            data=json.dumps({"sql": "SELECT 1"}).encode(),
            headers={"content-type": "application/json"},
        )
        with urlopen(request) as response:
            payload = json.loads(response.read())
        assert payload["result"]["rows"] == [{"conversation_count": 3}]
        assert payload["planned_sql"] == "planned: SELECT 1"
    finally:
        server.shutdown()
        server.server_close()


def test_query_rejects_empty_sql(monkeypatch, tmp_path):
    server = _start(tmp_path, monkeypatch)
    port = server.server_address[1]
    try:
        request = Request(
            f"http://127.0.0.1:{port}/api/query",
            data=json.dumps({"sql": "  "}).encode(),
            headers={"content-type": "application/json"},
        )
        with pytest.raises(HTTPError) as error:
            urlopen(request)
        assert error.value.code == 400
    finally:
        server.shutdown()
        server.server_close()


def test_query_refreshes_wren_for_every_database_request(monkeypatch, tmp_path):
    refreshed = []
    server = _start(
        tmp_path,
        monkeypatch,
        lambda database_id: refreshed.append(database_id) or FakeToolkit(),
    )
    port = server.server_address[1]
    try:
        for _ in range(2):
            request = Request(
                f"http://127.0.0.1:{port}/api/query",
                data=json.dumps(
                    {"sql": "SELECT 1", "database_id": "database-123"}
                ).encode(),
                headers={"content-type": "application/json"},
            )
            with urlopen(request) as response:
                assert response.status == 200
        assert refreshed == ["database-123", "database-123"]
    finally:
        server.shutdown()
        server.server_close()
