#!/bin/sh
set -eu

project="${WREN_PROJECT:-/app}"

if [ -n "${WREN_CONNECTION_FILE:-}" ]; then
  if [ -z "${WREN_PROFILE:-}" ]; then
    echo "WREN_PROFILE es obligatorio con WREN_CONNECTION_FILE" >&2
    exit 1
  fi
  wren profile add "$WREN_PROFILE" --from-file "$WREN_CONNECTION_FILE" --activate
elif [ "$project" = /app ] && ! wren profile debug leo-local >/dev/null 2>&1; then
  wren profile add leo-local --from-file /app/connection.example.yml --activate
fi

cd "$project"
wren context build
exec leo-wren --project "$project" "$@"
