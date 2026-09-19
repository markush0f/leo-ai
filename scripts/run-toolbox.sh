#!/usr/bin/env bash
# Build and run the MCP Toolbox container against Leo's Postgres.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
export LEO_UID="${LEO_UID:-$(id -u)}"
export LEO_GID="${LEO_GID:-$(id -g)}"
src="$root/third_party/mcp-toolbox"

if [[ ! -f "$src/go.mod" ]]; then
  echo "falta $src; inicializa el submódulo:" >&2
  echo "  git submodule update --init third_party/mcp-toolbox" >&2
  exit 1
fi

cd "$root"
docker compose up -d --build postgres toolbox
echo "toolbox en http://127.0.0.1:5000  (MCP_TOOLBOX_URL)"
