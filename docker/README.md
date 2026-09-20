# Docker

This folder holds one file per service.

| File | Purpose |
| --- | --- |
| `Dockerfile.backend` | Production backend (multi-stage cargo build) |
| `Dockerfile.cargo` | Dev backend (`cargo watch -C backend`) |
| `Dockerfile.web` | Frontend (`node:20`, dev + prod) |
| `Dockerfile.db` | Wrapper over `postgres:16-alpine` (optional) |
| `compose.db.yml` | **Database only** (Postgres) |
| `compose.test.yml` | **Isolated Postgres for the e2e suite** (never the dev DB) |
| `compose.cargo.yml` | Backend via `cargo watch` (needs db) |
| `compose.web.yml` | Frontend via `npm run dev` |
| `compose.yml` | Full production stack (`db` + `backend` + `web`) |
| `compose.dev.yml` | Full dev stack (`db` + `cargo` + `web`) |
| `.env` / `.env.example` / `.env.fish` | Prefilled env vars (bash + fish) |

## Dev workflow you asked for: DB on Docker, `cargo watch` + `npm run dev` on host

### 1. Start only the database

```sh
# bash / zsh
docker compose --env-file docker/.env -f docker/compose.db.yml up -d
# fish uses the same - the env file is read by compose, no sourcing needed
```

If `5432` is busy (see `docker ps` — another postgres), change `POSTGRES_PORT` in `docker/.env` to `5434` and keep `DATABASE_URL` as `postgres://alice:alice@127.0.0.1:5434/alice` (host) — the container still listens on `5432` internally.

### 2. Backend on host with live reload

```sh
# From repo root — cargo watch reads the env file itself, no `source` needed.
# -C backend is required: Cargo.toml lives at backend/Cargo.toml, not at repo root.
# Using -w backend alone fails with `error: project root does not exist`.

# bash / zsh / fish (all shells) — use --env-file, path is relative to backend workdir
cargo watch -C backend --env-file ../docker/.env -x 'run -p alice-daemon'

# or without --env-file (bash/zsh only):
# set -a; source docker/.env; set +a
# cargo watch -C backend -x 'run -p alice-daemon'

# or without --env-file (fish):
# source docker/.env.fish
# cargo watch -C backend -x 'run -p alice-daemon'

# If 5432 is busy on your machine (docker ps shows another postgres on 5432),
# edit POSTGRES_PORT in docker/.env to 5434 and keep DATABASE_URL as
# postgres://alice:alice@127.0.0.1:5434/alice — then restart the db:
# POSTGRES_PORT=5434 docker compose --env-file docker/.env -f docker/compose.db.yml up -d
```

Backend listens on `http://127.0.0.1:8787` and connects to `DATABASE_URL=postgres://alice:alice@127.0.0.1:5432/alice` (host) — compose sets `postgres://...@db:5432/...` automatically when backend runs inside Docker.

> **Why `-C backend`?** The workspace `Cargo.toml` lives at `backend/Cargo.toml`, not at the repo root. `cargo watch` without `-C` fails with `error: project root does not exist`. `-C backend` tells cargo to use `backend` as the project root. `-w backend` alone is not enough.

### 3. Frontend on host

```sh
npm run dev --prefix frontend-web
# web on http://localhost:5173, calls PUBLIC_ALICE_API_URL=http://127.0.0.1:8787
# frontend-web/.env is already prefilled — no extra env needed
```

### Test

```sh
curl http://127.0.0.1:8787/api/health
curl http://127.0.0.1:8787/api/v1/settings
curl http://127.0.0.1:8787/api/v1/conversations
curl -X POST http://127.0.0.1:8787/api/v1/chat -H 'content-type: application/json' -d '{"text":"hello"}'
# The reply carries the conversation. Read it with its full history:
# curl http://127.0.0.1:8787/api/v1/conversations/<id>
```

### End-to-end tests against an isolated DB

The web e2e suite starts its own backend and frontend. By default it uses a
sqlite file in `.tmp/` (no Docker). It stores real messages and flips
`app_setting`, so it must **not** point at the dev database. Use the
separate stack in `compose.test.yml` — a different name, port, and volume:

```sh
docker compose --env-file docker/.env -f docker/compose.test.yml up -d
DATABASE_URL=postgres://alice:alice@127.0.0.1:5435/alice_e2e npm --prefix frontend-web run test:e2e
docker compose --env-file docker/.env -f docker/compose.test.yml down -v  # wipe e2e data
```

The config refuses a `DATABASE_URL` that names the live `alice` database.
The suite waits for the daemon to compile, so a cold `target/` only makes
the first run slower. See `frontend-web/README.md` for the other knobs.

Stop DB: `docker compose -f docker/compose.db.yml down` (add `-v` to wipe `pg_data`).

## Fish notes

Your error `set: expected >= 1 arguments; got 0` and `Unsupported use of '='` comes from `set -a; source docker/.env` — that is **bash** syntax, not fish. In fish:

- Do **not** `source docker/.env` (bash format).
- Either `source docker/.env.fish` (fish format) or better `cargo watch --env-file docker/.env ...` which works in both shells.
- Compose already does ` --env-file docker/.env`, so `docker compose ...` never needs sourcing.

## Alternatives

Full dev in Docker (all three services):
```sh
docker compose --env-file docker/.env -f docker/compose.dev.yml up --build
```

Backend only in Docker (with DB):
```sh
docker compose --env-file docker/.env -f docker/compose.cargo.yml up --build
# cargo watch runs inside the container via Dockerfile.cargo
```

Frontend only in Docker:
```sh
docker compose --env-file docker/.env -f docker/compose.web.yml up --build
```

Full production:
```sh
docker compose --env-file docker/.env -f docker/compose.yml up --build -d
curl http://127.0.0.1:8787/api/health
curl http://127.0.0.1:3000  # web preview
```

## Env vars

| Var | Default | Used by |
| --- | --- | --- |
| `POSTGRES_DB` | `alice` | db |
| `POSTGRES_USER` | `alice` | db, backend |
| `POSTGRES_PASSWORD` | `alice` | db, backend |
| `POSTGRES_PORT` | `5432` | db host port |
| `DATABASE_URL` | `postgres://alice:alice@127.0.0.1:5432/alice` | backend (host) |
| `ALICE_PORT` | `8787` | backend host port |
| `ALICE_CORS_ORIGIN` | `http://localhost:5173` | backend |
| `ALICE_QUEUE_ENABLED` | `true` | backend default toggle |
| `RUST_LOG` | `info` | backend |
| `PUBLIC_ALICE_API_URL` | `http://127.0.0.1:8787` | frontend (`backend-client.ts`, `socket-client.ts`) |
| `WEB_PORT` | `5173` | frontend dev |
| `POSTGRES_E2E_DB` | `alice_e2e` | e2e test DB name (`compose.test.yml`) |
| `POSTGRES_E2E_USER` / `POSTGRES_E2E_PASSWORD` | `alice` | e2e test DB credentials |
| `POSTGRES_E2E_PORT` | `5435` | e2e test DB host port |

Compose files read `docker/.env` when you pass `--env-file docker/.env`. The same values are duplicated in `/.env` and `frontend-web/.env` for host tools that auto-load `.env` from their own directory. For fish, use `docker/.env.fish` or `cargo watch --env-file`.
