# Problems

## P0 - Distribution
- No installation method (cargo requires Rust toolchain - unrealistic for data engineers)
- Need: pre-built binaries, package managers (brew/apt), or Docker-based CLI
- Docs currently assume `ork` command exists but don't explain how to get it

## P1 - Examples
- ✅ FIXED - All examples now use typed Pydantic approach
  - Removed untyped dict-based code
  - Removed `__main__` testing blocks
  - Added pydantic to all pyproject.toml files
  - Separated business logic (src/) from Ork wrappers (tasks/)
  - Fixed imports to use `from src.module import...`
  - All examples tested and working: demo, parallel, branches, retries, quickstart

## P1 - Verbosity
- Typed approach requires 2 BaseModel classes even for trivial tasks
- Downstream tasks must redefine upstream output schemas
- High barrier to entry for simple workflows

## P2 - DX
- Generic `Input`/`Output` names make code unsearchable (every file has same class names)
- Should use prefixed names: `FetchInput`, `FetchOutput`

## P? - Python Executor
- ✅ FIXED - Now sets PYTHONPATH to project_root automatically
- Tasks can import from src/ using `from src.module import...`
