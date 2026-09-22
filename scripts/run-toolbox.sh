#!/usr/bin/env bash
# Build and run the MCP Toolbox container against Ira's Postgres.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
export IRA_UID="${IRA_UID:-$(id -u)}"
export IRA_GID="${IRA_GID:-$(id -g)}"
src="$root/third_party/mcp-toolbox"

if [[ ! -f "$src/go.mod" ]]; then
  echo "falta $src; inicializa el submódulo:" >&2
  echo "  git submodule update --init third_party/mcp-toolbox" >&2
  exit 1
fi

cd "$root"
docker compose up -d --build postgres toolbox
echo "toolbox en http://127.0.0.1:5000  (MCP_TOOLBOX_URL)"
