# Running Ork Locally

These are contributor/developer instructions for the full local stack (DB-backed scheduler + web UI).

## 1) Boot services (Postgres + Docker)

In terminal 1:

```bash
just up
```

What this does:
- Starts Postgres via `docker compose`
- Runs migrations
- Starts the DB-backed scheduler (`ork run` with no workflow file)
- Starts the web UI at `http://127.0.0.1:4000`

Leave this running in one terminal.

## 1b) Boot services (SQLite, no Docker)

```bash
just up-sqlite
```

This runs the scheduler and web UI against `sqlite://./.ork/ork.db?mode=rwc` without Docker.
Set `DATABASE_URL` to use a different SQLite file (e.g. `sqlite://./tmp/ork.db?mode=rwc`).

## 2) Run an example

In terminal 2:

```bash
just example simple
```

This runs `examples/workflows/simple/simple.yaml` via `ork execute` using the local SQLite DB by default.

## Optional

- Change the DB connection: `DATABASE_URL=postgres://... just up`
- List all workflows: `cargo run -p ork-cli --bin ork -- list-workflows`
