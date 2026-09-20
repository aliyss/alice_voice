# Alice Voice

A local assistant for a desktop. You write a message, the daemon reads the
intent of it with a pipeline of small readers, reads the values the intent
needs, and runs the shell command the intent carries. Nothing leaves the
machine: the intent is read by a llama.cpp server you run, or by models the
daemon downloads itself.

The repository holds two projects and the files that run them:

| Path | What it is |
| --- | --- |
| `backend/` | The Rust workspace: `alice-core` (config, DTOs, shared types) and `alice-daemon` (the API, the resolver, the queue, the command runner) |
| `frontend-web/` | The Qwik City web app: the transcript, the aura, and the settings page |
| `docker/` | One file per service, and the compose stacks (database only, dev, test, production) |
| `guidelines/` | The rules the code follows. Start at [`guidelines/GUIDELINES.md`](guidelines/GUIDELINES.md) |
| `shell.nix`, `.envrc` | The development shell, with the CUDA libraries a built in model needs on a graphics card |

The web app talks to the daemon over REST and one WebSocket. It never talks
to a database or a model itself, and the daemon never serves a page.

## A turn, end to end

1. `POST /api/v1/chat` carries the text. The daemon takes it into the intake
   queue, handles it while the request runs, and moves the handled turn into
   the conversation store. A message always belongs to a conversation, and
   the conversation keeps the full history that is read as the context of
   the next turn. With the queue off, the daemon answers without storing the
   turn at all.
2. The resolver reads the message. `resolver_backend` picks the engine, and
   the layered router is the one the settings page draws: a deterministic
   pass, a retrieval that keeps a short list, a decision that chooses one of
   it, and an extraction that reads the values of the chosen intent.
3. The entity values are read for the intent: an open value the user says, a
   closed list the intent owns, or a script that answers with a live list.
   A required value the daemon could not read stops the turn and asks for it.
4. The command of the intent runs with the values in place, and the reply,
   the exit code, and the metadata of the whole turn are stored on both
   messages. The metadata names the engine, the model, the route stage by
   stage, and the reader behind every value.
5. `GET /api/v1/events` streams the same steps while the turn runs, which is
   what the aura in the web app draws.

## Requirements

- **Rust** — a stable toolchain (the crates are edition 2021). `cargo fmt`,
  `clippy`, and `cargo test` are what CI runs.
- **Node 20 or newer** — `frontend-web/` is a Qwik City app with Vite.
- **A database** — SQLite by default, no service to start. Postgres 16 is
  supported and the compose files carry one.
- **A model server (optional)** — an OpenAI compatible llama.cpp server for
  the generative readers. The deterministic pass, the words of the catalog,
  and the built in models need no server at all.
- **Nix (optional)** — `shell.nix` provides the CUDA runtime libraries for a
  daemon built with `--features gliner-cuda`. Without it the built in models
  run on the processor.

## Setup

### 1. Quickstart on SQLite

The shortest path: no Docker, no model server, one terminal each.

```sh
# 1. The daemon, on the port the web app expects. SQLite is the default
#    database, so there is nothing to start first, and the file lands in
#    the directory you run it from.
cd backend
ALICE_LISTEN_ADDR=0.0.0.0:8788 cargo run -p alice-daemon
#    -> http://127.0.0.1:8788, the schema is created and migrated on boot

# 2. The web app, in a second terminal.
cd frontend-web
npm install
npm run dev
#    -> http://localhost:5173, calling the daemon on 8788 (frontend-web/.env)
```

From the repository root the daemon is one command:

```sh
ALICE_LISTEN_ADDR=0.0.0.0:8788 \
  cargo run -p alice-daemon --manifest-path backend/Cargo.toml
```

**On the port.** With no environment at all the daemon listens on `8787` and
uses SQLite. The env files of the repository — `docker/.env`, `/.env`, and
`frontend-web/.env` — put the daemon on `8788` and point the web app at it,
so a dev stack stands beside a bare `cargo run` instead of fighting it for
the port. This file uses `8788` in its examples:
`PUBLIC_ALICE_API_URL` is read when the app is built, so a daemon the app
does not expect is a request that reaches nothing.

### 2. Postgres instead of SQLite

```sh
docker compose --env-file docker/.env -f docker/compose.db.yml up -d

# Host tools read the URL of the container from the port it publishes.
set -a; source docker/.env; set +a

cargo watch -C backend -x 'run -p alice-daemon'
```

The compose file publishes Postgres on `POSTGRES_PORT` (`5434` in
`docker/.env`), so a Postgres already running on `5432` does not collide.
See [`docker/README.md`](docker/README.md) for the fish variants, the
database-only workflow, and the full stacks.

### 3. A model server (only for the generative readers)

The stock settings read the intent and the values with a model server, so a
fresh database answers with the model server down and says so on the
settings page. Point the daemon at a llama.cpp server that speaks the OpenAI
API (`/v1/chat/completions` and `/v1/embeddings`):

```sh
export ALICE_RESOLVER_BASE_URL=http://127.0.0.1:8012/v1
export ALICE_RESOLVER_MODEL=qwen3.5-4b
```

Both values are database settings once the settings page has been used, so
the environment only decides where a fresh database starts. The settings
page offers the models the server reports.

### 4. The first run in the browser

Open <http://localhost:5173/settings/>. The **Intent resolver** section draws
the pipeline as a graph and holds the settings of every stage:

- The **Model server** block names the address and the chat model, and shows
  whether the server answers.
- The **Built in models** and **Built in GLiNER** blocks list the models, the
  size of each download, and whether it is on disk. Download, choose, and
  delete them there.
- The **The message** block holds the sentences you really say. Play one, or
  play all of them, and read the route the message took before a turn
  depends on it: the reading names what the daemon would do, and **Debug**
  names every value it reported, stage by stage. Nothing runs and nothing is
  stored but the list of sentences itself.
- The **Intents** section is the catalog: the name, the phrases a user might
  say, the entities with the values they take, and the command.

## Configuration

The settings page is the source of truth. Every value it writes is stored in
the database and read by the next turn, so a running daemon does not have to
be restarted. The environment decides the values a **fresh** database starts
from, and the settings that no turn reads (the listen address, CORS).

`backend/config/example.toml` documents the shape of the configuration with
the same defaults, but the daemon reads the environment only. There is no
config file to load.

| Variable | Default | What it does |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite://alice.db?mode=rwc` | The database. Postgres: `postgres://user:pass@host:port/db` |
| `ALICE_LISTEN_ADDR` | `0.0.0.0:8787` (`8788` in the env files) | The address the API listens on |
| `ALICE_CORS_ORIGIN` | `http://localhost:5173` | The origin the app runs on. The router answers any origin today, so this value is documentation |
| `ALICE_QUEUE_ENABLED` | `true` | Whether the daemon stores a turn. Off answers without a conversation |
| `ALICE_MAX_CONNECTIONS` | `5` | Connections of the database pool |
| `ALICE_RESOLVER_BASE_URL` | `http://127.0.0.1:8012/v1` | The model server |
| `ALICE_RESOLVER_MODEL` | `qwen3.5-4b` | The model the server answers to |
| `ALICE_RESOLVER_BACKEND` | `hybrid` | `router`, `llama`, `gliner`, or `hybrid` |
| `ALICE_RESOLVER_TIMEOUT_SECS` | `30` | How long one model request may take |
| `ALICE_RESOLVER_MAX_TOKENS` | `512` | The largest answer the resolver reads |
| `ALICE_RESOLVER_CONTEXT_TURNS` | `6` | Earlier turns read as the context of a turn |
| `ALICE_RESOLVER_THINKING` | `false` | Whether the model may reason before it answers |
| `ALICE_GLINER_MODEL` | `gliner_small-v2.1` | The built in model |
| `ALICE_GLINER_DEVICE` | `auto` | `auto`, `cpu`, or `cuda` |
| `ALICE_GLINER_THRESHOLD` | `0.3` | Smallest probability a label needs |
| `ALICE_GLINER_MODELS_DIR` | `models/gliner` | Where the downloaded models live |
| `ALICE_GLINER_THREADS` | `4` | Threads one inference may use |
| `ALICE_EXECUTION_ENABLED` | `true` | Whether the command of an intent runs |
| `ALICE_EXECUTION_TIMEOUT_SECS` | `20` | How long a command may run |
| `ALICE_ENTITY_SCRIPT_*` | `3`, `60`, `200` | The timeout, the cache, and the largest list of a script entity |
| `ALICE_ROUTER_*` | see the graph | The layered router: `FAST_PATH`, `RETRIEVE`, `DECIDE`, `EXTRACT`, `TOP_K`, `FLOOR`, `MARGIN`, the two weights, the embedding source and models, `LOCAL_DEVICE`, `PHRASE_GATE`, `LIST_MATCH`, and `LIST_FLOOR` |
| `RUST_LOG` | `info` | The log filter. `tracing_subscriber` syntax, so per module is allowed |
| `PUBLIC_ALICE_API_URL` | `http://127.0.0.1:8788` in `frontend-web/.env` | The web app's daemon. Read at build time |
| `ALICE_GLINER_CUDA` | set by `shell.nix` | Whether the shell makes CUDA possible |

`docker/.env` holds these for the compose stacks, and `/.env` and
`frontend-web/.env` duplicate them for host tools that look in their own
directory.

## Commands

```sh
# Backend
cd backend
cargo fmt                        # format
cargo clippy --all-targets -- -D warnings  # lint, warnings are errors
cargo test --workspace           # the unit and integration tests
cargo build --release -p alice-daemon
cargo run -p alice-daemon --features gliner-cuda  # built in models on the card

# Web app
cd frontend-web
npm run dev          # the dev server with HMR
npm run build        # type check, lint, and build
npm run lint         # eslint
npm run fmt.check    # prettier
npm run build.types  # tsc --noEmit
npm run test         # the unit tests (vitest)
npm run test:e2e     # the browser suite (Playwright)
```

Before a change is done, the full check is `cargo fmt`, `cargo clippy
--all-targets -- -D warnings`, `cargo test --workspace`, and on the web side
`npm run fmt.check`, `npm run lint`, `npx tsc --noEmit`, `npm run test`, and
`npm run test:e2e`.

### End-to-end tests

`frontend-web/playwright.config.ts` starts its own daemon and its own preview
build, so no manual setup is needed beyond Rust and the Playwright browsers.
The suite is self-contained on a throwaway SQLite file at `.tmp/e2e.db` and
its own port `8790`, so it never touches a running dev daemon.

```sh
cd frontend-web
npm run test:e2e          # chromium, headless
npm run test:e2e:ui       # watch mode
npm run test:e2e:debug    # step through
```

The suite writes real messages and flips `app_setting`, so it refuses a
Postgres `DATABASE_URL` that names the live database. To run it against
Postgres, use the isolated stack instead:

```sh
docker compose --env-file docker/.env -f docker/compose.test.yml up -d
DATABASE_URL=postgres://alice:alice@127.0.0.1:5435/alice_e2e npm run test:e2e
docker compose --env-file docker/.env -f docker/compose.test.yml down -v
```

## Debugging

### Logs

The daemon logs through `tracing`, filtered by `RUST_LOG`:

```sh
RUST_LOG=alice_daemon=debug cargo run -p alice-daemon
RUST_LOG=alice_daemon=trace,tower_http=debug cargo run -p alice-daemon
```

A turn logs which stage answered, which reader ran, whether a stage fell back
(a model that did not answer, an embedding server that is down), and the
command with its exit code. Warning level is where a fallback is reported, so
`RUST_LOG=warn` alone already says when the pipeline is running on less than
the settings promise.

### Read the state by hand

The daemon answers for itself, so a shell is often faster than the browser:

```sh
curl http://127.0.0.1:8788/api/health      # is it up
curl http://127.0.0.1:8788/api/v1/status   # the state, the start time, the version
curl http://127.0.0.1:8788/api/v1/settings # every stored setting
curl http://127.0.0.1:8788/api/v1/dependencies
#   -> the database, the model server, and the built in model apart: which
#      one answers, and what the daemon would use
curl http://127.0.0.1:8788/api/v1/resolver
#   -> the devices this build offers, the models on disk, the label budget
curl http://127.0.0.1:8788/api/v1/conversations
curl http://127.0.0.1:8788/api/v1/conversations/<id>
#   -> the full history, with the metadata of every turn: the engine, the
#      model, the route, and the reader behind every value
```

Read one sentence the way a turn reads it, without running anything:

```sh
curl -X POST http://127.0.0.1:8788/api/v1/resolver/preview \
  -H 'content-type: application/json' -d '{"text":"open firefox"}'
```

Send a turn and read the reply:

```sh
curl -X POST http://127.0.0.1:8788/api/v1/chat \
  -H 'content-type: application/json' -d '{"text":"what is the weather"}'
```

The WebSocket at `/api/v1/events` carries the steps of a running turn. In a
browser console:

```js
const socket = new WebSocket('ws://127.0.0.1:8788/api/v1/events');
socket.onmessage = (event) => console.log(JSON.parse(event.data));
```

### The settings page as a diagnostic

The page is built to answer "why did that not work" without a log:

- Every field says whether its place answers. A value that cannot take effect
  is disabled and names the reason — the database does not answer, the model
  server is down, the built in model is not on disk. The **Model server**
  block names the address the daemon really resolved.
- The **The message** block reads a sentence through the stored settings and
  draws the route on the graph, colored by how each stage ended: green for a
  stage that answered, amber for a stage that fell back, red for the stage
  that refused. **Debug** names every reader, candidate score, value, and
  duration behind that answer.
- The **Intents** section warns when the label budget of a built in model is
  passed (20 labels is comfortable, 30 is the limit), and says why the
  catalog cannot be changed when a write would not take effect. The script
  of a script entity can be run from the form, so the list a turn would read
  is read before an intent depends on it.

### The database

```sh
sqlite3 backend/alice.db '.tables'
sqlite3 backend/alice.db 'select text, intent_id from chat_message order by created_at desc limit 5;'
sqlite3 backend/alice.db 'select key, value from app_setting;'
```

With Postgres, replace the first argument with the URL from `docker/.env`.
Settings live one row per key in `app_setting`, the conversations in
`conversation`, the turns in `chat_message` (where `meta` holds the JSON
metadata of a turn), the catalog in `intent`, `intent_entity`,
`intent_entity_value`, and `intent_example`, and a queued message in
`message_queue` until it is handled. The daemon adds the columns and tables
a database it created earlier is missing on boot.

### When something does not work

| Symptom | Where to look |
| --- | --- |
| The daemon exits on boot | The first `ERROR` line: the database URL is usually wrong, or the address is taken (`ALICE_LISTEN_ADDR`) |
| Every field on the settings page is disabled | The database does not answer. `/api/v1/dependencies` names it, and the page says which one |
| The reply is always "I could not match that to an intent" | Read the route: refused below the floor means the catalog does not hold the message, and a red stage means the stage that should have answered could not read |
| A turn falls back to the words | The model server or the built in model did not answer. `RUST_LOG=warn` reports the fallback and the reason |
| The intent is right and a value stays empty | Read the value fields of the conversation: `source`, `read`, and `score` say which reader read what, and a list entity says which entry it matched |
| The graph shows no route | The stored `resolver_backend` is not `router`, so the turn was read by a single engine. Preview sentences report a route only for the router |
| The web app cannot reach the daemon | `PUBLIC_ALICE_API_URL` is read when the app is built, so a change needs a restart of the dev server. The API answers any origin, so CORS is not the cause |
| The e2e suite refuses to run | It guards the live database. Point `DATABASE_URL` at `alice_e2e` or let it use SQLite |
| A built in model offers no `CUDA` | The daemon is not built with `--features gliner-cuda`, or the shell did not provide the CUDA libraries. `/api/v1/resolver` lists the devices the build really offers |

## Where to read next

- [`guidelines/GUIDELINES.md`](guidelines/GUIDELINES.md) — the guide to the guidelines
- [`guidelines/backend/ARCHITECTURE.md`](guidelines/backend/ARCHITECTURE.md) — the daemon: the resolver, the route, the metadata, the store
- [`guidelines/frontend/web/CODE_STRUCTURE.md`](guidelines/frontend/web/CODE_STRUCTURE.md) — the web app
- [`frontend-web/README.md`](frontend-web/README.md) — the web app in detail: the aura, the transcript, the settings page, the e2e knobs
- [`docker/README.md`](docker/README.md) — every compose stack
