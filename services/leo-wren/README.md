# Leo Wren

Datasource-agnostic runner for official Wren AI SDK. It uses `wren-langchain` directly; no separate Wren server is required. Bundled project models Leo's local PostgreSQL schema, while `--project` can load any Wren project and supported datasource.

Exposed MDL models: `conversations`, `messages`, `providers`, and `models`. Secret columns (`providers.api_key`, `secrets`, database credentials, tool payloads) are deliberately omitted.

## Requirements

- Python 3.11-3.13. Wren AI does not currently support Python 3.14.
- A database supported by Wren: PostgreSQL, MySQL, BigQuery, Snowflake, ClickHouse, Trino, MSSQL, Databricks, Redshift, Spark, Athena, Oracle, or DuckDB.
- Wren CLI and SDK installed by this project's dependencies.

## Install

From this directory:

```sh
python3.13 -m venv .venv
. .venv/bin/activate
pip install -e '.[dev]'
cp .env.example .env
```

Only bundled example needs Leo's local database:

```sh
docker compose up -d postgres
```

## Configure Bundled Project

Profiles live in `~/.wren/profiles.yml`; credentials remain in this project's ignored `.env`. Create profile once from provided placeholder-only template:

```sh
wren docs connection-info postgres
wren profile add leo-local --from-file connection.example.yml
```

Template references `POSTGRES_HOST`, `POSTGRES_PORT`, `POSTGRES_DATABASE`, `POSTGRES_USER`, and `POSTGRES_PASSWORD`; Wren resolves them from `.env` at runtime without copying secrets into profile.

Build source MDL into `target/mdl.json`:

```sh
wren context build
```

## Generate Project From Rust JSON

`leo-pgjson` includes schema metadata, primary keys, and foreign keys in its
JSON output. Generate all Wren YAML files from that dump:

```sh
cargo run -p leo-pgjson -- --url "$DATABASE_URL" --schema-only --out /tmp/database.json
leo-wren generate /tmp/database.json --output services/leo-wren --profile leo-local
wren context build
```

Generation replaces `models/` and writes `wren_project.yml` and
`relationships.yml`. Connection credentials remain separate. Sensitive tables
and columns are excluded unless `--include-sensitive` is explicitly supplied.
Containers can generate on startup by mounting this schema-only JSON and setting
`WREN_SCHEMA_JSON` to its container path.

## Test connection

From repository root, complete setup and smoke test with one command:

```sh
./scripts/start-leo-wren.sh
```

The launcher creates `.venv` and `.env` when missing, starts PostgreSQL, creates
the `leo-local` profile, builds the MDL, and runs `leo-wren check`. It accepts
the same commands as `leo-wren`:

When Python 3.11-3.13 is unavailable on host, launcher builds and runs provided
Python 3.13 container automatically. Wren profile persists in `leo-wren-home`
Docker volume.

```sh
./scripts/start-leo-wren.sh query 'SELECT role, COUNT(*) AS total FROM messages GROUP BY role'
```

For manual execution inside this directory:

```sh
leo-wren check
```

Expected output includes `"status": "ok"`, SQL translated to selected dialect, and connectivity result.

Run other MDL queries:

```sh
leo-wren dry-plan 'SELECT channel, COUNT(*) AS total FROM conversations GROUP BY channel' --database-id "$DATABASE_ID"
leo-wren query 'SELECT channel, COUNT(*) AS total FROM conversations GROUP BY channel' --database-id "$DATABASE_ID"
leo-wren query 'SELECT role, COUNT(*) AS total FROM messages GROUP BY role' --database-id "$DATABASE_ID"
```

Queries reference MDL model names, not physical `schema.table` names.

## Use Another Database

Wren binds dialect and schema to a project, so each database needs its own `wren_project.yml`, model metadata, and matching connection profile. The runner itself is not tied to PostgreSQL.

```sh
export WREN_PROJECT=/path/to/my-wren-project
export WREN_PROFILE=my-database
export WREN_CONNECTION_FILE=/path/to/connection.yml
export WREN_CHECK_SQL='SELECT 1 AS connection_ok'
export WREN_START_LOCAL_POSTGRES=false
./scripts/start-leo-wren.sh check
```

`WREN_CONNECTION_FILE` is optional when profile already exists in `~/.wren/profiles.yml`. `WREN_PROFILE` is optional when project declares `profile:` or active Wren profile should be used. Direct CLI invocation supports same selection:

```sh
leo-wren --project /path/to/my-wren-project --profile my-database query 'SELECT * FROM MyModel LIMIT 10'
```

Connection file format depends on datasource. Inspect required fields with:

```sh
wren docs connection-info mysql
wren docs connection-info bigquery
wren docs connection-info snowflake
```

## Browser

```sh
./scripts/start-leo-wren.sh serve
```

Opens http://127.0.0.1:8778/ with the same query and dry-plan commands. The page is `web/index.html`.

## Natural-language test

Install model provider integration and set its API key in `.env`:

```sh
pip install -e '.[agent]'
leo-wren ask 'Cuantas conversaciones hay por canal?'
```

Override model with `--model` or `WREN_AGENT_MODEL`, using any LangChain-supported provider installed in environment.
Pass `--database-id UUID` to `ask`; immediately before every question Leo fetches
fresh schema JSON from `leo-server`, regenerates YAML, and rebuilds Wren context.

## Tests

```sh
pytest
```

Unit tests mock Wren runtime and need no database. `leo-wren check` is real integration smoke test.
