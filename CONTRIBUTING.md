# Contributing

Ideas and bug reports are welcome as [issues](https://github.com/Edgar-Mendonca/Datum-Plan/issues/new). File › Roadmap in the app (and `docs/site.json`) shows what is planned.

## How the code is laid out

- `crates/core`: the engine in pure Rust (model, calendars, scheduling, resources, costs, reports, file formats). All logic lives here, with tests.
- `src-tauri`: the desktop shell (commands, files, updater).
- `ui/src`: the window (SolidJS + TypeScript). It shows what the engine computes and sends edits back as operations.
- `docs/MEMORY.md`: architecture, decisions, roadmap and a log of every session. Read it first.

## Before you send a change

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # engine tests, including the hardening tests
npm run check:ui                  # type-check the window
npm run build:ui && node tests_ui/run.cjs   # UI tests (Playwright)
cargo deny check                  # licences, advisories, sources (cargo install cargo-deny)
```

Keep code compact and in the style around it. A new engine feature comes with a test; a new window feature with a UI test where it can.

By contributing you agree your work is released under the MIT licence.
