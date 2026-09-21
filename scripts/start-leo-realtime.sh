#!/usr/bin/env bash
# Start Leo Realtime with Docker Compose and wait until it is healthy.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
port="${LEO_REALTIME_PORT:-8765}"

for command in docker curl; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'falta el comando requerido: %s\n' "$command" >&2
    exit 1
  fi
done

if ! docker compose version >/dev/null 2>&1; then
  printf 'Docker Compose no está disponible.\n' >&2
  exit 1
fi

export LEO_UID="${LEO_UID:-$(id -u)}"
export LEO_GID="${LEO_GID:-$(id -g)}"
export LEO_REALTIME_MODE="${LEO_REALTIME_MODE:-leo}"

printf 'Arrancando Leo Realtime en modo %s...\n' "$LEO_REALTIME_MODE"
docker compose --project-directory "$root" up -d --build leo-realtime

deadline=$((SECONDS + 180))
until curl -fsS "http://127.0.0.1:$port/healthz" >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'Leo Realtime no respondió antes del timeout.\n' >&2
    docker compose --project-directory "$root" ps leo-realtime
    exit 1
  fi
  sleep 1
done

printf 'Leo Realtime listo: http://127.0.0.1:%s/test\n' "$port"
