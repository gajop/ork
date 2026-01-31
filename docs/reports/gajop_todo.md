DONE?
1. Architecture.md is too abstract. The Components should have actual ork names, not be these very generic things that any could be used to describe 99% of orchestrators there. The idea of this would be to introduce terminology/project components and show how they interact in the diagram. Same for Data Flow and other diagrams there.
2. crates.md has too much code and talks too much about dependencies, stuff that is normally clearly shown in Cargo.toml I think? It doesn't successfully summarize this information.
	- On the second level, per crate, you may want to have a document that goes into more details about this crate. This can be either inside the crate itself or at this level



4. This is a big problem, please implement it: No DAG support (tasks run independently)
6. Needs to be implemented: No scheduled runs (manual trigger only)

----

AI TODO?

# Problems

## P0 - Distribution
- No installation method (cargo requires Rust toolchain - unrealistic for data engineers)
- Need: pre-built binaries, package managers (brew/apt), or Docker-based CLI
- Docs currently assume `ork` command exists but don't explain how to get it

## P1 - Verbosity
- Typed approach requires 2 BaseModel classes even for trivial tasks
- Downstream tasks must redefine upstream output schemas
- High barrier to entry for simple workflows

## P2 - DX
- Generic `Input`/`Output` names make code unsearchable (every file has same class names)
- Should use prefixed names: `FetchInput`, `FetchOutput`
