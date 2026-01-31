# Running Ork Locally

This workflow uses a single command to boot the database, scheduler, and web UI, plus a separate command to trigger an example workflow.

## 1) Boot services

```bash
just up
```

What this does:
- Starts Postgres via `docker compose`
- Runs migrations
- Starts the DB-backed scheduler (`ork run`)
- Starts the web UI at `http://127.0.0.1:4000`

Leave this running in one terminal.

## 2) Run an example

In a second terminal:

```bash
just example-run simple
```

This creates the workflow from `examples/simple/simple.yaml`, triggers it, and polls for status.

## Optional

- Change the DB connection: `DATABASE_URL=postgres://... just up`
- List all workflows: `cargo run -p ork-cli --bin ork -- list-workflows`
