from __future__ import annotations

import argparse
import json
import os
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Callable, Sequence
from urllib.parse import urlparse
from urllib.request import urlopen

from .generator import WrenGenerator


DEFAULT_CHECK_SQL = os.environ.get("WREN_CHECK_SQL", "SELECT 1 AS connection_ok")
DEFAULT_LEO_API = os.environ.get("LEO_API_URL", "http://127.0.0.1:8787").rstrip("/")


def discover_project(cwd: Path, module_file: Path) -> Path:
    """Find the Wren project from the working directory, then from this file.

    An installed copy lives under site-packages, so walking a fixed number of
    parents from ``__file__`` does not reach ``wren_project.yml``.
    """
    seen: set[Path] = set()
    for start in (cwd, module_file.resolve().parent):
        for candidate in (start, *start.parents):
            if candidate in seen:
                continue
            seen.add(candidate)
            if (candidate / "wren_project.yml").is_file():
                return candidate
    return cwd


DEFAULT_PROJECT = discover_project(Path.cwd(), Path(__file__))


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="leo-wren",
        description="Query any datasource supported by a Wren AI project.",
    )
    parser.add_argument(
        "--project",
        type=Path,
        default=Path(os.environ.get("WREN_PROJECT", DEFAULT_PROJECT)),
        help="Wren project directory (default: this service)",
    )
    parser.add_argument(
        "--profile",
        default=os.environ.get("WREN_PROFILE"),
        help="Wren connection profile (default: project or active profile)",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    check = subparsers.add_parser("check", help="Plan and execute a safe smoke query")
    check.add_argument("--sql", default=DEFAULT_CHECK_SQL)
    check.add_argument("--database-id")

    query = subparsers.add_parser("query", help="Execute MDL SQL through Wren")
    query.add_argument("sql")
    query.add_argument("--database-id")

    dry_plan = subparsers.add_parser(
        "dry-plan", help="Translate MDL SQL without executing it"
    )
    dry_plan.add_argument("sql")
    dry_plan.add_argument("--database-id")

    ask = subparsers.add_parser(
        "ask", help="Ask a natural-language question using a LangChain agent"
    )
    ask.add_argument("question")
    ask.add_argument("--database-id")
    ask.add_argument(
        "--model",
        default=os.environ.get("WREN_AGENT_MODEL", "openai:gpt-4o-mini"),
        help="LangChain model identifier",
    )

    serve = subparsers.add_parser("serve", help="Open the local query page")
    serve.add_argument("--host", default="127.0.0.1")
    serve.add_argument("--port", type=int, default=8778)

    generate = subparsers.add_parser(
        "generate", help="Generate Wren YAML files from a leo-pgjson dump"
    )
    generate.add_argument("json", type=Path, help="Path to the JSON dump")
    generate.add_argument("--output", type=Path, help="Wren project destination")
    generate.add_argument("--name", help="Wren project name")
    generate.add_argument("--profile", dest="generate_profile", default="leo-local")
    generate.add_argument(
        "--include-sensitive",
        action="store_true",
        help="Include credential and tool-payload tables/columns",
    )
    return parser


def load_toolkit(project: Path, profile: str | None = None) -> Any:
    from wren_langchain import WrenToolkit

    options = {"profile": profile} if profile else {}
    return WrenToolkit.from_project(project.resolve(), **options)


def refresh_project(
    project: Path,
    profile: str | None,
    database_id: str,
    api_url: str = DEFAULT_LEO_API,
) -> None:
    with urlopen(f"{api_url}/api/databases/{database_id}/schema", timeout=60) as response:
        dump = json.load(response)
    WrenGenerator(dump, profile=profile or "leo-local").generate(project)
    subprocess.run(
        ["wren", "context", "build"],
        cwd=project,
        check=True,
        capture_output=True,
        text=True,
    )


class WrenRuntime:
    def __init__(self, project: Path, profile: str | None) -> None:
        self.project = project
        self.profile = profile
        self._lock = threading.Lock()

    def toolkit_for(self, database_id: str) -> Any:
        with self._lock:
            refresh_project(self.project, self.profile, database_id)
            return load_toolkit(self.project, self.profile)


def table_payload(table: Any) -> dict[str, Any]:
    return {
        "columns": list(table.column_names),
        "row_count": table.num_rows,
        "rows": table.to_pylist(),
    }


def page_path(project: Path) -> Path:
    return project.resolve() / "web" / "index.html"


def make_server(
    project: Path,
    toolkit: Any,
    host: str,
    port: int,
    refresh_toolkit: Callable[[str], Any] | None = None,
) -> ThreadingHTTPServer:
    page = page_path(project).read_bytes()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            path = urlparse(self.path).path
            if path == "/":
                self._bytes(200, page, "text/html; charset=utf-8")
                return
            if path == "/api/health":
                self._json(200, {"status": "ok"})
                return
            self._json(404, {"error": "no encontrado"})

        def do_POST(self) -> None:
            path = urlparse(self.path).path
            if path not in {"/api/query", "/api/plan"}:
                self._json(404, {"error": "no encontrado"})
                return
            try:
                length = int(self.headers.get("content-length", "0"))
                raw = self.rfile.read(length)
                body = json.loads(raw) if raw else {}
                sql = body["sql"]
                if not isinstance(sql, str) or not sql.strip():
                    raise ValueError("sql vacío")
                request_toolkit = toolkit
                if refresh_toolkit is not None:
                    database_id = body["database_id"]
                    if not isinstance(database_id, str) or not database_id.strip():
                        raise ValueError("database_id vacío")
                    request_toolkit = refresh_toolkit(database_id.strip())
            except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
                self._json(400, {"error": f"petición inválida: {error}"})
                return
            except Exception as error:
                self._json(502, {"error": f"no se pudo actualizar Wren: {error}"})
                return
            if path == "/api/plan":
                self._reply(self._plan(request_toolkit, sql))
                return
            self._reply(self._query(request_toolkit, sql))

        def _check(self) -> tuple[int, dict[str, Any]]:
            try:
                return 200, {
                    "status": "ok",
                    "planned_sql": toolkit.dry_plan(DEFAULT_CHECK_SQL),
                    "result": table_payload(toolkit.query(DEFAULT_CHECK_SQL)),
                }
            except Exception as error:
                return 500, {"error": str(error)}

        def _plan(self, active_toolkit: Any, sql: str) -> tuple[int, dict[str, Any]]:
            try:
                return 200, {"planned_sql": active_toolkit.dry_plan(sql)}
            except Exception as error:
                return 400, {"error": str(error)}

        def _query(self, active_toolkit: Any, sql: str) -> tuple[int, dict[str, Any]]:
            try:
                return 200, {
                    "planned_sql": active_toolkit.dry_plan(sql),
                    "result": table_payload(active_toolkit.query(sql)),
                }
            except Exception as error:
                return 400, {"error": str(error)}

        def _reply(self, payload: tuple[int, dict[str, Any]]) -> None:
            status, body = payload
            self._json(status, body)

        def _json(self, status: int, body: dict[str, Any]) -> None:
            encoded = json.dumps(body, default=str).encode()
            self._bytes(status, encoded, "application/json; charset=utf-8")

        def _bytes(self, status: int, body: bytes, content_type: str) -> None:
            self.send_response(status)
            self.send_header("content-type", content_type)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, fmt: str, *args: Any) -> None:
            print(f"{self.address_string()} {fmt % args}", flush=True)

    return ThreadingHTTPServer((host, port), Handler)


def serve(project: Path, profile: str | None, host: str, port: int) -> int:
    runtime = WrenRuntime(project, profile)
    server = make_server(project, None, host, port, runtime.toolkit_for)
    shown = "127.0.0.1" if host in {"0.0.0.0", "::"} else host
    print(f"Leo Wren en http://{shown}:{server.server_address[1]}/", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print(flush=True)
    finally:
        server.server_close()
    return 0


def run(args: argparse.Namespace) -> int:
    if args.command == "generate":
        dump = json.loads(args.json.read_text(encoding="utf-8"))
        output = args.output or args.project
        WrenGenerator(
            dump,
            name=args.name,
            profile=args.generate_profile,
            include_sensitive=args.include_sensitive,
        ).generate(output)
        print(f"Wren project generated in {output.resolve()}")
        return 0

    if args.command == "serve":
        return serve(args.project, args.profile, args.host, args.port)

    database_id = getattr(args, "database_id", None)
    if database_id:
        refresh_project(args.project, args.profile, database_id)
    toolkit = load_toolkit(args.project, args.profile)

    if args.command == "dry-plan":
        print(toolkit.dry_plan(args.sql))
        return 0

    if args.command == "query":
        print(json.dumps(table_payload(toolkit.query(args.sql)), default=str, indent=2))
        return 0

    if args.command == "check":
        planned_sql = toolkit.dry_plan(args.sql)
        result = table_payload(toolkit.query(args.sql))
        print(
            json.dumps(
                {"status": "ok", "planned_sql": planned_sql, "result": result},
                default=str,
                indent=2,
            )
        )
        return 0

    try:
        from langchain.agents import create_agent
    except ImportError as error:
        raise RuntimeError(
            "Install the agent extra first: pip install -e '.[agent]'"
        ) from error

    tools = toolkit.get_tools(include_memory_write=False, raise_on_error=True)
    agent = create_agent(
        model=args.model,
        tools=tools,
        system_prompt=toolkit.system_prompt(tools=tools),
    )
    response = agent.invoke(
        {"messages": [{"role": "user", "content": args.question}]}
    )
    print(response["messages"][-1].content)
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    return run(create_parser().parse_args(argv))


if __name__ == "__main__":
    raise SystemExit(main())
