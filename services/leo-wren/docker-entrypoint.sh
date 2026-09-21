#!/bin/sh
set -eu

project="${WREN_PROJECT:-/app}"
dynamic=false
for argument in "$@"; do
  if [ "$argument" = "serve" ] || [ "$argument" = "--database-id" ]; then
    dynamic=true
  fi
done

if [ -n "${WREN_SCHEMA_JSON:-}" ]; then
  leo-wren generate "$WREN_SCHEMA_JSON" --output "$project" \
    --profile "${WREN_PROFILE:-leo-local}"
elif [ ! -f "$project/wren_project.yml" ] && [ "$dynamic" = false ]; then
  echo "falta wren_project.yml; monta WREN_SCHEMA_JSON para generarlo" >&2
  exit 1
fi

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
if [ -f wren_project.yml ]; then
  wren context build
fi
exec leo-wren --project "$project" "$@"
