#!/bin/sh
set -eu
real=/usr/bin/docker.bin
if [ "${1:-}" = "compose" ] && [ "${2:-}" = "up" ]; then
  shift 2
  exec "$real" compose up --no-recreate "$@"
fi
exec "$real" "$@"
