#!/usr/bin/env bash
# Run the MCP Toolbox container against Ira's Postgres.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
export IRA_UID="${IRA_UID:-$(id -u)}"
export IRA_GID="${IRA_GID:-$(id -g)}"
cd "$root"
docker compose up -d postgres toolbox
echo "toolbox en http://127.0.0.1:5000  (MCP_TOOLBOX_URL)"
