# Running Ork Locally

This workflow uses a single command to boot the database, scheduler, and web UI, plus a separate command to trigger an example workflow.

## 1) Boot services (Postgres + Docker)

```bash
just up
```

What this does:
- Starts Postgres via `docker compose`
- Runs migrations
- Starts the DB-backed scheduler (`ork run`)
- Starts the web UI at `http://127.0.0.1:4000`

Leave this running in one terminal.

## 1b) Boot services (SQLite, no Docker)

```bash
just up-sqlite
```

This runs the scheduler and web UI against `sqlite://./.ork/ork.db?mode=rwc` without Docker.
Set `DATABASE_URL` to use a different SQLite file (e.g. `sqlite://./tmp/ork.db?mode=rwc`).

## 2) Run an example

In a second terminal:

```bash
just example simple
```

This uses `ork run-workflow` to post YAML to the running API, trigger a run, and poll status.
Set `ORK_API_URL` (or pass `--api-url`) if the web UI is running on a different host/port.

### Example run

```bash
just example simple
```

## Optional

- Change the DB connection: `DATABASE_URL=postgres://... just up`
- List all workflows: `cargo run -p ork-cli --bin ork -- list-workflows`
