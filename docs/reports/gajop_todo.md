1. Architecture.md is too abstract. The Components should have actual ork names, not be these very generic things that any could be used to describe 99% of orchestrators there. The idea of this would be to introduce terminology/project components and show how they interact in the diagram. Same for Data Flow and other diagrams there.
2. crates.md has too much code and talks too much about dependencies, stuff that is normally clearly shown in Cargo.toml I think? It doesn't successfully summarize this information.
	- On the second level, per crate, you may want to have a document that goes into more details about this crate. This can be either inside the crate itself or at this level



4. This is a big problem, please implement it: No DAG support (tasks run independently)
5. Why do we have this still? ## ork-runner (Legacy - Deprecated)
6. No scheduled runs (manual trigger only)
