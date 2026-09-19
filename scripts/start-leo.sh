#!/usr/bin/env bash
# Start Leo's local services, HTTP API, and web frontend.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
desktop="$root/desktop"
export LEO_UID="${LEO_UID:-$(id -u)}"
export LEO_GID="${LEO_GID:-$(id -g)}"

for command in docker cargo npm curl; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'falta el comando requerido: %s\n' "$command" >&2
    exit 1
  fi
done

if ! docker compose version >/dev/null 2>&1; then
  printf 'Docker Compose no está disponible.\n' >&2
  exit 1
fi

if [[ ! -f "$root/third_party/mcp-toolbox/go.mod" ]]; then
  printf 'falta third_party/mcp-toolbox; ejecuta:\n' >&2
  printf '  git submodule update --init third_party/mcp-toolbox\n' >&2
  exit 1
fi

printf 'Arrancando Postgres y MCP Toolbox...\n'
docker compose --project-directory "$root" up -d --build postgres toolbox

deadline=$((SECONDS + 120))
until docker compose --project-directory "$root" exec -T postgres \
  pg_isready -U leo -d leo >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'Postgres no respondió antes del timeout.\n' >&2
    docker compose --project-directory "$root" ps
    exit 1
  fi
  sleep 1
done

deadline=$((SECONDS + 120))
until curl -fsS http://127.0.0.1:5000/healthz >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'MCP Toolbox no respondió antes del timeout.\n' >&2
    docker compose --project-directory "$root" ps
    exit 1
  fi
  sleep 1
done

if [[ ! -d "$desktop/node_modules" ]]; then
  printf 'Instalando dependencias del frontend...\n'
  npm --prefix "$desktop" ci
fi

export MCP_TOOLBOX_URL="${MCP_TOOLBOX_URL:-http://127.0.0.1:5000}"

printf '\nLeo listo:\n'
printf '  Frontend  http://127.0.0.1:${LEO_DEV_PORT:-5179}\n'
printf '  API       http://127.0.0.1:${LEO_HTTP_PORT:-8787}\n'
printf '  Toolbox   http://127.0.0.1:5000\n\n'

exec npm --prefix "$desktop" run web
