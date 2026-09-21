# Vikunja

Leo uses [Vikunja](https://vikunja.io/) as the external backend for tasks and
reminders. This directory only runs the official image. It does not vendor
Vikunja's source, and Leo does not talk to it yet.

The host reaches it at `http://127.0.0.1:3456`. Another container attached to
this Compose network can use `http://vikunja:3456`. The port is published on
the loopback interface only.

## Start

From this directory:

```sh
docker compose up -d
```

Defaults are `VIKUNJA_PORT=3456` and
`VIKUNJA_PUBLIC_URL=http://localhost:3456/`. Copy `.env.example` to `.env`
when you want to change them. `.env` is gitignored.

## Stop

```sh
docker compose down
```

`down` removes the container and keeps the named volumes.

## Logs and status

```sh
docker compose logs -f vikunja
docker compose ps
```

A healthy process answers `GET /health` with `OK` and serves the web UI at
`http://127.0.0.1:3456/`. `GET /api/v1/info` returns the version. Compose
marks the container healthy with the image's `vikunja healthcheck` command
(v2.6.0). The image has no `curl` or `wget`.

## Data

SQLite is the database. Vikunja stores it at `/db/vikunja.db` inside the
container. Uploads live at `/app/vikunja/files`.

Both paths are named Docker volumes:

| Volume | Container path | Contents |
| --- | --- | --- |
| `vikunja-db` | `/db` | SQLite database |
| `vikunja-files` | `/app/vikunja/files` | Uploads and other Vikunja files |

Docker prefixes those names with the Compose project name (`vikunja` when you
run Compose from this directory). Recreating or replacing the container does
not delete the volumes, so the database and files come back on the next
`docker compose up -d`.

The official image runs as user `1000` and contains no shell. Docker creates
a new named volume as root, so `vikunja-init` (a one-shot Alpine container)
sets both volumes to `1000:1000` before Vikunja starts. It does not store data.

## Reset the development instance

This deletes the local database and every uploaded file:

```sh
docker compose down -v
```

Deleting the Docker volumes removes Vikunja's local data. There is no copy
elsewhere in the Leo repository.
