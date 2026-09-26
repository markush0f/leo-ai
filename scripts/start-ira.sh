#!/usr/bin/env bash
# Start Ira's local services, HTTP API, WhatsApp bridge, and web frontend.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
desktop="$root/desktop"
whatsapp="$root/services/ira-whatsapp"
export IRA_UID="${IRA_UID:-$(id -u)}"
export IRA_GID="${IRA_GID:-$(id -g)}"
realtime_port="${IRA_REALTIME_PORT:-8765}"
export IRA_REALTIME_MODE="${IRA_REALTIME_MODE:-ira}"

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

printf 'Arrancando Postgres, MCP Toolbox, Ira Realtime y Veritas Kanban...\n'
docker compose --project-directory "$root" --profile kanban up -d --build \
  postgres toolbox ira-realtime veritas-kanban veritas-mcp

deadline=$((SECONDS + 120))
until docker compose --project-directory "$root" exec -T postgres \
  pg_isready -U ira -d ira >/dev/null 2>&1; do
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

deadline=$((SECONDS + 180))
until curl -fsS http://127.0.0.1:3001/health >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'Veritas Kanban no respondió antes del timeout.\n' >&2
    docker compose --project-directory "$root" --profile kanban ps
    exit 1
  fi
  sleep 1
done

deadline=$((SECONDS + 120))
until curl -sS -o /dev/null -X POST http://127.0.0.1:3100/mcp \
  -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
  --data '{}'; do
  if (( SECONDS >= deadline )); then
    printf 'Veritas MCP no respondió antes del timeout.\n' >&2
    docker compose --project-directory "$root" --profile kanban ps
    exit 1
  fi
  sleep 1
done

deadline=$((SECONDS + 120))
until curl -fsS "http://127.0.0.1:$realtime_port/healthz" >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'Ira Realtime no respondió antes del timeout.\n' >&2
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
export VERITAS_MCP_URL="${VERITAS_MCP_URL:-http://127.0.0.1:3100}"
api_port="${IRA_HTTP_PORT:-8787}"
api_bind="${IRA_HTTP_BIND:-127.0.0.1:$api_port}"
api_health="http://127.0.0.1:$api_port/api/health"
api_pid=""
whatsapp_port="${WHATSAPP_CONTROL_PORT:-8790}"
whatsapp_pid=""
export IRA_API_URL="${IRA_API_URL:-http://127.0.0.1:$api_port}"
export WHATSAPP_CONTROL_PORT="$whatsapp_port"

stop_pid() {
  local pid="$1"
  if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" 2>/dev/null || true
  fi
}

cleanup() {
  stop_pid "$whatsapp_pid"
  stop_pid "$api_pid"
}
trap cleanup EXIT INT TERM

if ! curl -fsS "$api_health" >/dev/null 2>&1; then
  printf 'Arrancando ira-server...\n'
  cargo run -p ira-server -- --bind "$api_bind" &
  api_pid=$!

  deadline=$((SECONDS + 180))
  until curl -fsS "$api_health" >/dev/null 2>&1; do
    if ! kill -0 "$api_pid" >/dev/null 2>&1; then
      wait "$api_pid"
      exit $?
    fi
    if (( SECONDS >= deadline )); then
      printf 'ira-server no respondió antes del timeout.\n' >&2
      exit 1
    fi
    sleep 1
  done
fi

whatsapp_status="http://127.0.0.1:$whatsapp_port/status"
if ! curl -fsS "$whatsapp_status" >/dev/null 2>&1; then
  if [[ ! -d "$whatsapp/node_modules" ]]; then
    printf 'Instalando dependencias de WhatsApp...\n'
    npm --prefix "$whatsapp" ci
  fi
  printf 'Arrancando puente WhatsApp...\n'
  npm --prefix "$whatsapp" start &
  whatsapp_pid=$!

  deadline=$((SECONDS + 60))
  until curl -fsS "$whatsapp_status" >/dev/null 2>&1; do
    if ! kill -0 "$whatsapp_pid" >/dev/null 2>&1; then
      wait "$whatsapp_pid"
      exit $?
    fi
    if (( SECONDS >= deadline )); then
      printf 'el puente de WhatsApp no respondió antes del timeout.\n' >&2
      exit 1
    fi
    sleep 1
  done
fi

printf '\nIra listo:\n'
printf '  Frontend  http://127.0.0.1:%s\n' "${IRA_DEV_PORT:-5179}"
printf '  API       http://127.0.0.1:%s\n' "$api_port"
printf '  WhatsApp  http://127.0.0.1:%s\n' "$whatsapp_port"
printf '  Toolbox   http://127.0.0.1:5000\n'
printf '  Realtime  http://127.0.0.1:%s/test\n\n' "$realtime_port"

cd "$desktop"
npm exec vite -- --host 127.0.0.1 --port "${IRA_DEV_PORT:-5179}" --strictPort
