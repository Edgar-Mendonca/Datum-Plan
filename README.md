<div align="center">

<img src="docs/brand/datum-plan-icon.svg" width="88" alt="Datum Plan">

# Datum Plan

**Project scheduling for Windows: the strengths of Microsoft Project and Primavera P6 in one small, fast app.**

[Download for Windows](https://github.com/Edgar-Mendonca/Datum-Plan/releases/latest) · [What's new](CHANGELOG.md) · [Project memory and roadmap](docs/MEMORY.md)

</div>

<img src="docs/images/gantt.png" alt="Datum Plan Gantt chart" width="100%">

## Install

1. Download `DatumPlan-x.y.z-setup.exe` from [Releases](https://github.com/Edgar-Mendonca/Datum-Plan/releases/latest).
2. Run it. It installs for your user account, with no administrator rights needed. The installer is not code-signed yet, so on the first install Windows may show *"Windows protected your PC"*. Click **More info › Run anyway**.
3. Later versions install themselves. Open **File › Home** (or click the amber **Update available** badge) and click **Update now**.

## What it does today

- An MS Project-style ribbon, task sheet and Gantt chart, in a light theme.
- A view sidebar: Gantt, Task List, Calendar, Dashboard and Reports.
- Drag Gantt bars to move them, drag the end to resize, drag down to link.
- Summary tasks, milestones, and FS/SS/FF/SF links with lags. A Rust critical path engine calculates total and free float.
- A dashboard with an S-curve, and weekly or monthly progress reports you can print or save as PDF.
- Five templates (engineering, EPC, software, building, research), undo and redo, `.dplan` files.
- One-click updates from GitHub Releases, with SHA-256 checksum verification.

<img src="docs/images/dashboard.png" alt="Dashboard" width="100%">

See the [changelog](CHANGELOG.md) for details and the [roadmap](docs/MEMORY.md#roadmap) for what comes next.

## Build from source

You need Rust (stable) and Node.js 20+. On Linux, also install `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev`.

```bash
npm ci
npm run dev        # run the app
cargo test         # engine tests
npm run build      # Windows installer (on Windows)
```

## Release a new version

1. Bump `version` in `src-tauri/Cargo.toml` (and `crates/core/Cargo.toml`).
2. Add a `## x.y.z — date` section at the top of `CHANGELOG.md`. It becomes the release notes and the in-app "What's new".
3. `git tag vx.y.z && git push origin vx.y.z`. GitHub Actions builds the installer and publishes the release.

## Licence

MIT © 2026 Edgar Mendonca
