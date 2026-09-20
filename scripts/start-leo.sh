#!/usr/bin/env bash
# Start Leo's local services, HTTP API, and web frontend.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
desktop="$root/desktop"
export LEO_UID="${LEO_UID:-$(id -u)}"
export LEO_GID="${LEO_GID:-$(id -g)}"
realtime_port="${LEO_REALTIME_PORT:-8765}"
export LEO_REALTIME_MODE="${LEO_REALTIME_MODE:-leo}"

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

printf 'Arrancando Postgres, MCP Toolbox y Leo Realtime...\n'
docker compose --project-directory "$root" up -d --build postgres toolbox leo-realtime

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

deadline=$((SECONDS + 120))
until curl -fsS "http://127.0.0.1:$realtime_port/healthz" >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'Leo Realtime no respondió antes del timeout.\n' >&2
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
api_port="${LEO_HTTP_PORT:-8787}"
api_bind="${LEO_HTTP_BIND:-127.0.0.1:$api_port}"
api_health="http://127.0.0.1:$api_port/api/health"
api_pid=""

cleanup() {
  if [[ -n "$api_pid" ]] && kill -0 "$api_pid" >/dev/null 2>&1; then
    kill "$api_pid" >/dev/null 2>&1 || true
    wait "$api_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

if ! curl -fsS "$api_health" >/dev/null 2>&1; then
  printf 'Arrancando leo-server...\n'
  cargo run -p leo-server -- --bind "$api_bind" &
  api_pid=$!

  deadline=$((SECONDS + 180))
  until curl -fsS "$api_health" >/dev/null 2>&1; do
    if ! kill -0 "$api_pid" >/dev/null 2>&1; then
      wait "$api_pid"
      exit $?
    fi
    if (( SECONDS >= deadline )); then
      printf 'leo-server no respondió antes del timeout.\n' >&2
      exit 1
    fi
    sleep 1
  done
fi

printf '\nLeo listo:\n'
printf '  Frontend  http://127.0.0.1:%s\n' "${LEO_DEV_PORT:-5179}"
printf '  API       http://127.0.0.1:%s\n' "$api_port"
printf '  Toolbox   http://127.0.0.1:5000\n'
printf '  Realtime  http://127.0.0.1:%s/test\n\n' "$realtime_port"

cd "$desktop"
npm exec vite -- --host 127.0.0.1 --port "${LEO_DEV_PORT:-5179}" --strictPort
