# RustPaint: notes for Claude

RustPaint ports HaikuPaint (`~/Projects/haiku/HaikuPaint`) to Rust for macOS,
Linux and Windows. **Read `PLAN.md` first**: it has the architecture, the
crate choices, the milestones and where everything reusable lives.

- HaikuPaint is the specification; port its behavior, don't redesign it.
- Keep the engine crate pure (no GUI dependencies) and run `cargo test`
  after every engine change.
- MacPaint's artwork is © Apple and comes only from the extractor. The CHM
  source in HaikuPaint's `reference/` is for study: never copy it, and never
  commit anything from a `reference/` folder.
- Commit only when the user asks.
