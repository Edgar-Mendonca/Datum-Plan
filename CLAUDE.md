# Datum Plan: notes for Claude

Read `docs/MEMORY.md` first. It is the project's memory: architecture, decisions, roadmap, and a log of every session.
At the end of every session that changes anything, update `docs/MEMORY.md`:
- tick the roadmap
- add a session-log entry
- record any new decisions
- refresh the architecture section if files moved

User preferences:
- Rust and Tauri 2, small and efficient code, Windows first.
- Ship in small numbered releases (0.1.0, 0.1.1, …). Every release appears in the in-app updater.
- A light theme with blue accents (since 0.1.1), MS Project layout, P6 power features, Cairn-style sidebar.
- Keep code compact and match the surrounding style. Engine logic belongs in `crates/core` (pure Rust, unit-tested), not in the UI.
- The UI is 7 script files in `ui-src/` sharing one global scope, joined by tsc in the order set in `ui-src/tsconfig.json`.

Checks before pushing:
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm run check:ui`

Releasing:
1. Bump the version in `src-tauri/Cargo.toml` and `crates/core/Cargo.toml`.
2. Add a CHANGELOG section.
3. Push the tag `vX.Y.Z`, or run `release.yml` with workflow_dispatch when tag pushes are blocked. The workflow creates the tag and publishes the installer.
