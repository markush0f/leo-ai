#!/usr/bin/env bash
# Start Ira's local services, HTTP API, and web frontend.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
desktop="$root/desktop"
export IRA_UID="${IRA_UID:-$(id -u)}"
export IRA_GID="${IRA_GID:-$(id -g)}"
realtime_port="${IRA_REALTIME_PORT:-8765}"
export IRA_REALTIME_MODE="${IRA_REALTIME_MODE:-ira}"
ira_home="${HOME}/.ira"
mkdir -p "$ira_home"
token_file="$ira_home/http.token"
if [[ -z "${IRA_HTTP_TOKEN:-}" ]]; then
  if [[ ! -s "$token_file" ]]; then
    umask 077
    openssl rand -hex 32 >"$token_file"
    chmod 600 "$token_file"
  fi
  IRA_HTTP_TOKEN="$(tr -d '[:space:]' <"$token_file")"
  export IRA_HTTP_TOKEN
fi

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

printf 'Arrancando Postgres, MCP Toolbox e Ira Realtime...\n'
docker compose --project-directory "$root" up -d --build \
  postgres toolbox ira-realtime

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

wait_healthy() {
  local service="$1"
  local label="$2"
  local timeout="$3"
  local deadline=$((SECONDS + timeout))
  local cid status
  until cid="$(docker compose --project-directory "$root" ps -q "$service")" \
    && [[ -n "$cid" ]] \
    && status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$cid")" \
    && [[ "$status" == "healthy" ]]; do
    if (( SECONDS >= deadline )); then
      printf '%s no respondió antes del timeout.\n' "$label" >&2
      docker compose --project-directory "$root" ps
      exit 1
    fi
    sleep 1
  done
}

wait_healthy ira-realtime "Ira Realtime" 120

if [[ ! -d "$desktop/node_modules" ]]; then
  printf 'Instalando dependencias del frontend...\n'
  npm --prefix "$desktop" ci
fi

export MCP_TOOLBOX_URL="${MCP_TOOLBOX_URL:-http://127.0.0.1:5000}"
api_port="${IRA_HTTP_PORT:-8787}"
api_bind="${IRA_HTTP_BIND:-127.0.0.1:$api_port}"
api_health="http://127.0.0.1:$api_port/api/health"
api_pid=""
export IRA_API_URL="${IRA_API_URL:-http://127.0.0.1:$api_port}"

stop_pid() {
  local pid="$1"
  if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" 2>/dev/null || true
  fi
}

cleanup() {
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

printf '\nIra listo:\n'
printf '  Frontend  http://127.0.0.1:%s\n' "${IRA_DEV_PORT:-5179}"
printf '  API       http://127.0.0.1:%s\n' "$api_port"
printf '  Toolbox   http://127.0.0.1:5000\n'
printf '  Realtime  http://127.0.0.1:%s/test\n\n' "$realtime_port"

cd "$desktop"
npm exec vite -- --host 127.0.0.1 --port "${IRA_DEV_PORT:-5179}" --strictPort
