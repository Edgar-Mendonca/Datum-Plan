# Changelog

## 0.1.0 — 2026-10-24

The first version: the application shell, the scheduling engine and automatic updates.

**Install and update**
- Windows installer for your user account, with no administrator rights needed.
- The Home screen shows your version and checks GitHub for new releases. **Update now** downloads the new version, checks its SHA-256 checksum, installs it and restarts. You never need to uninstall.
- An **Update available** badge appears in the title bar when a new version is out.

**Plan**
- A task sheet in the style of MS Project: ID, Task Name, Duration, Start, Finish, Predecessors, % Complete and Total Float.
- Type straight into the grid. Typing a name in an empty row adds a task.
- Durations: `5`, `5d`, `2w`, `1mo`. Predecessors: `3`, `3SS+2d`, `5FF-1d`.
- Summary tasks: indent and outdent (Alt+Shift+→/←), with collapse and expand.
- Link and unlink tasks (Ctrl+F2 / Ctrl+Shift+F2) with finish-to-start, start-to-start, finish-to-finish and start-to-finish links and lags.
- Typing a Start date sets a "start no earlier than" constraint.
- Task Information (Shift+F2) and Project Information (name, start date, working days).
- Undo and redo (Ctrl+Z / Ctrl+Y), up to 200 steps.

**Schedule**
- A Rust critical path engine with forward and backward passes, total and free float, a working-day calendar and summary rollups.
- Circular links are refused with a clear message.

**Gantt chart**
- Bars, summary bars, milestones, progress, link arrows, critical path in red, weekends and a today line.
- Two-tier timescale. Zoom with the status bar slider, Ctrl+wheel or View › Entire Project.

**Files**
- Save and open `.dplan` files (Ctrl+S / Ctrl+O). Double-clicking a `.dplan` file opens it.
- Recent plans on the Home screen. If you close with unsaved changes, Datum Plan asks before closing.
- A sample plan: a centrifugal compressor package from kick-off to shipment.

**Known limits in 0.1**
- Durations are whole working days on a single Monday–Friday calendar; hours and holidays arrive in 0.2.
- Summary tasks cannot be linked. Indenting a task under a linked task removes that task's links.
- Excel, CSV, Project XML and XER import and export are planned for 0.3.
