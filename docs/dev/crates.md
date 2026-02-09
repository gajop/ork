# Crates

This is a compressed map of the workspace so you do not have to read code to orient yourself. For deeper details, see per-crate notes under [docs/dev/crates/](crates/) (and [crates/ork-cli/README.md](../../crates/ork-cli/README.md) for the CLI).

## Workspace Map

| Crate             | What it owns                                          | Details                                                     |
| ----------------- | ----------------------------------------------------- | ----------------------------------------------------------- |
| **ork-cli**       | CLI, scheduler host process, migrations, perf tooling | [docs/dev/crates/ork-cli.md](crates/ork-cli.md)             |
| **ork-core**      | Domain models + traits + scheduler loop               | [docs/dev/crates/ork-core.md](crates/ork-core.md)           |
| **ork-state**     | Database implementations of `ork-core::Database`      | [docs/dev/crates/ork-state.md](crates/ork-state.md)         |
| **ork-executors** | Executor backends + `ExecutorManager`                 | [docs/dev/crates/ork-executors.md](crates/ork-executors.md) |
| **ork-web**       | Axum UI/API backed by the DB                          | [docs/dev/crates/ork-web.md](crates/ork-web.md)             |
| **ork-worker**    | Worker process for compile/execute paths              | [docs/dev/crates/ork-worker.md](crates/ork-worker.md)       |
| **ork-sdk-rust**  | Rust SDK for task payload contracts                   | [docs/dev/crates/ork-sdk-rust.md](crates/ork-sdk-rust.md)   |

## Dependency Direction

- [ork-core](crates/ork-core.md) is the base layer (traits + models + scheduler).
- [ork-state](crates/ork-state.md) and [ork-executors](crates/ork-executors.md) implement `ork-core` traits.
- [ork-cli](crates/ork-cli.md) wires everything together and runs the scheduler loop.
- [ork-web](crates/ork-web.md) reads and writes Postgres state via [ork-state](crates/ork-state.md).
- [ork-worker](crates/ork-worker.md) executes isolated worker duties backed by [ork-core](crates/ork-core.md) and [ork-executors](crates/ork-executors.md).
- [ork-sdk-rust](crates/ork-sdk-rust.md) is consumed by user task code and is intentionally independent from runtime orchestration crates.

## Where to go next

- Want to understand the scheduler loop? Start in [docs/dev/crates/ork-core.md](crates/ork-core.md).
- Want to understand storage? Start in [docs/dev/crates/ork-state.md](crates/ork-state.md).
- Want to add a new executor? Start in [docs/dev/crates/ork-executors.md](crates/ork-executors.md).
