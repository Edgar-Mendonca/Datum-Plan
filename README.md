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

- **MS Project look, P6 power:** ribbon, task sheet and Gantt chart, a timeline strip, and a Primavera-style details pane (General, Relationships, Resources, Notes).
- **Scheduling to the minute:** calendars with working hours and holidays, task calendars, all constraints and deadlines, manual and inactive tasks, FS/SS/FF/SF links with lags in hours or days, and critical path with total and free float.
- **Progress:** status date, actual dates, remaining duration, Mark on Track, baselines with variance and baseline bars.
- **Resources:** a resource sheet (work, material and cost), assignments, costs, and workload with overallocation.
- **Views:** Gantt, Task List, Calendar, Resources, Workload, Dashboard (S-curve) and weekly/monthly Reports.
- **Exchange:** import and export MS Project XML, Primavera P6 XER, Excel and CSV.
- **Speed:** any column you need, cut/copy/paste (also from Excel), fill down, find/replace, Ctrl+K command search, Ctrl+J quick add, and recovery of unsaved work.
- **Updates:** one-click updates from GitHub Releases, with SHA-256 checksum verification.

<img src="docs/images/details.png" alt="Details pane with relationships" width="100%">
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
