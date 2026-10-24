# Changelog

## 0.1.1 — 2026-10-24

A lighter look, a view sidebar, templates, a Gantt chart you can drag, and progress reporting.

**New look**
- A light theme with white panels and blue accents in place of the dark navy chrome.
- A new, lighter logo.
- A view sidebar on the left: Gantt, Task List, Calendar, Dashboard and Reports. Network, Resources and Workload show the version they arrive in.

**Templates instead of the sample plan**
- File › Home offers Engineering design, EPC project, Software development, Building construction and Research project, each with a miniature Gantt preview. Templates start next Monday.

**Gantt you can touch**
- Drag a bar to move it. This sets a "start no earlier than" date.
- Drag the end of a bar to change its duration.
- Drag a bar down onto another task to link them (finish-to-start).
- Hover over a bar to see its dates, duration, % complete and float.
- The Finish column can now be edited. Typing a finish date sets the duration.
- The weekly timescale shows day numbers when zoomed out.

**Views and reports**
- Task List: the full-width task sheet.
- Calendar: a month view of starts, finishes and milestones. Click a task to jump to it in the Gantt.
- Dashboard: % complete against planned, finish date, task counts, late and critical tasks, a progress S-curve, phase progress and upcoming milestones.
- Reports: a weekly or monthly progress report. It lists what finished, what is due, what starts, milestones, work in progress, late tasks and a look-ahead to the next period. Print it or save it as PDF.
- Late tasks are shown in red in the task sheet.

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
