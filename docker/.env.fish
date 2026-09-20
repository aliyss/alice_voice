# Fish-compatible env for Alice Voice. Source with: source docker/.env.fish
# Or just use `cargo watch --env-file docker/.env` which works without sourcing.
set -gx POSTGRES_DB alice
set -gx POSTGRES_USER alice
set -gx POSTGRES_PASSWORD alice
set -gx POSTGRES_PORT 5434
set -gx DATABASE_URL postgres://alice:alice@127.0.0.1:5434/alice
set -gx ALICE_LISTEN_ADDR 0.0.0.0:8788
set -gx ALICE_PORT 8788
set -gx ALICE_CORS_ORIGIN http://localhost:5173
set -gx ALICE_QUEUE_ENABLED true
set -gx RUST_LOG info
set -gx WEB_PORT 5173
set -gx PUBLIC_ALICE_API_URL http://127.0.0.1:8788
