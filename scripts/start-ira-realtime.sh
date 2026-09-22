#!/usr/bin/env bash
# Start Ira Realtime with Docker Compose and wait until it is healthy.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
port="${IRA_REALTIME_PORT:-8765}"

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

export IRA_UID="${IRA_UID:-$(id -u)}"
export IRA_GID="${IRA_GID:-$(id -g)}"
export IRA_REALTIME_MODE="${IRA_REALTIME_MODE:-ira}"

printf 'Arrancando Ira Realtime en modo %s...\n' "$IRA_REALTIME_MODE"
docker compose --project-directory "$root" up -d --build ira-realtime

deadline=$((SECONDS + 180))
until curl -fsS "http://127.0.0.1:$port/healthz" >/dev/null 2>&1; do
  if (( SECONDS >= deadline )); then
    printf 'Ira Realtime no respondió antes del timeout.\n' >&2
    docker compose --project-directory "$root" ps ira-realtime
    exit 1
  fi
  sleep 1
done

printf 'Ira Realtime listo: http://127.0.0.1:%s/test\n' "$port"
