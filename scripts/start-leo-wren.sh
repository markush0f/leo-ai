#!/usr/bin/env bash
# Prepare Wren AI and run its database smoke test or a supplied CLI command.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
service="$root/services/leo-wren"
project="${WREN_PROJECT:-$service}"
venv="$service/.venv"
image="leo-ai-wren:0.1.0"

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    printf 'falta el comando requerido: docker\n' >&2
    exit 1
  fi
}

if [[ ! -f "$service/.env" ]]; then
  cp "$service/.env.example" "$service/.env"
  printf 'Creado %s desde valores locales de ejemplo.\n' "$service/.env"
fi

start_postgres="${WREN_START_LOCAL_POSTGRES:-}"
if [[ -z "$start_postgres" && "$project" == "$service" ]]; then
  start_postgres=true
fi
if [[ "$start_postgres" == true ]]; then
  require_docker
  if ! docker compose version >/dev/null 2>&1; then
    printf 'Docker Compose no está disponible.\n' >&2
    exit 1
  fi
  printf 'Arrancando PostgreSQL local...\n'
  docker compose --project-directory "$root" up -d postgres

  deadline=$((SECONDS + 60))
  until docker compose --project-directory "$root" exec -T postgres \
    pg_isready -U leo -d leo >/dev/null 2>&1; do
    if (( SECONDS >= deadline )); then
      printf 'PostgreSQL no respondió antes del timeout.\n' >&2
      exit 1
    fi
    sleep 1
  done
fi

if (( $# == 0 )); then
  set -- check
fi

dynamic=false
for argument in "$@"; do
  if [[ "$argument" == serve || "$argument" == --database-id ]]; then
    dynamic=true
  fi
done

python_bin="${PYTHON_BIN:-}"
if [[ -z "$python_bin" ]]; then
  for candidate in python3.13 python3.12 python3.11; do
    if command -v "$candidate" >/dev/null 2>&1; then
      python_bin="$candidate"
      break
    fi
  done
fi

if [[ -n "$python_bin" && ( ! -x "$venv/bin/leo-wren" || "$service/pyproject.toml" -nt "$venv/bin/leo-wren" ) ]]; then
  if [[ ! -x "$venv/bin/python" ]]; then
    printf 'Creando entorno Python para Leo Wren...\n'
    "$python_bin" -m venv "$venv"
  fi
  printf 'Instalando dependencias de Leo Wren...\n'
  "$venv/bin/python" -m pip install -e "$service"
fi

if [[ ! -x "$venv/bin/leo-wren" ]]; then
  require_docker
  printf 'Python 3.11-3.13 no disponible; usando contenedor Python 3.13...\n'
  docker build --tag "$image" "$service"
  docker_args=(
    --rm
    --network host
    --env-file "$service/.env"
    --volume leo-wren-home:/root/.wren
  )
  if [[ "$project" != "$service" ]]; then
    docker_args+=(
      --volume "$project:/workspace/wren-project"
      --env WREN_PROJECT=/workspace/wren-project
    )
  fi
  if [[ -n "${WREN_PROFILE:-}" ]]; then
    docker_args+=(--env "WREN_PROFILE=$WREN_PROFILE")
  fi
  if [[ -n "${WREN_CHECK_SQL:-}" ]]; then
    docker_args+=(--env "WREN_CHECK_SQL=$WREN_CHECK_SQL")
  fi
  if [[ -n "${WREN_CONNECTION_FILE:-}" ]]; then
    docker_args+=(
      --volume "$WREN_CONNECTION_FILE:/workspace/connection.yml:ro"
      --env WREN_CONNECTION_FILE=/workspace/connection.yml
    )
  fi
  exec docker run "${docker_args[@]}" "$image" "$@"
fi

if [[ -n "${WREN_CONNECTION_FILE:-}" ]]; then
  if [[ -z "${WREN_PROFILE:-}" ]]; then
    printf 'WREN_PROFILE es obligatorio con WREN_CONNECTION_FILE.\n' >&2
    exit 1
  fi
  (
    cd "$project"
    "$venv/bin/wren" profile add "$WREN_PROFILE" \
      --from-file "$WREN_CONNECTION_FILE" \
      --activate
  )
elif [[ "$project" == "$service" ]] && ! (
  cd "$service"
  "$venv/bin/wren" profile debug leo-local >/dev/null 2>&1
); then
  printf 'Creando perfil Wren leo-local...\n'
  (
    cd "$service"
    "$venv/bin/wren" profile add leo-local \
      --from-file "$service/connection.example.yml" \
      --activate
  )
fi

if [[ -f "$project/wren_project.yml" ]]; then
  (
    cd "$project"
    "$venv/bin/wren" context build
  )
elif [[ "$dynamic" != true ]]; then
  printf 'Falta wren_project.yml; usa serve o --database-id para generarlo.\n' >&2
  exit 1
fi
exec "$venv/bin/leo-wren" --project "$project" "$@"
