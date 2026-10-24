# Changelog

## 0.2.0 — 2026-10-24

A big step towards MS Project and Primavera P6: real calendars, constraints, resources, progress tracking, baselines, file exchange, and a lot more control in the grid and the Gantt chart.

**Scheduling engine (P6-style, to the minute)**
- Durations in minutes, hours, days, weeks or months (`4h`, `2.5d`, `2w`, `1mo`). Lags take units too (`3SS+4h`).
- Calendars with working hours per day (for example 08:00–12:00 and 13:00–17:00), holidays and special working days. A plan can have several calendars, and each task can use its own.
- All constraints: As Soon/Late As Possible, Start/Finish No Earlier/Later Than, Must Start/Finish On. Deadlines, with negative float when they are missed.
- Manually scheduled tasks, inactive tasks, and links to and from summary tasks.
- A status (data) date: completed work keeps its actual dates and remaining work moves after the status date.
- Actual start and finish, remaining duration, and Mark on Track. Setting % complete fills in the actual dates.
- Milestones sit at their predecessor's finish, as in MS Project.

**Resources and costs**
- Resource Sheet: work, material and cost resources, groups, max units, standard rate, cost per use and material labels.
- Assign resources in the Resource Names column (`Engineer[50%], Concrete[12]`) or in the details pane. New names are added to the sheet.
- Work, cost and overallocation are calculated. The Workload view shows hours per week or per day, with over-capacity in red.

**Baselines and tracking**
- Set or clear a baseline for the whole project or for selected tasks.
- Baseline bars in the Gantt chart, baseline columns, start and finish variance, and a baseline curve on the dashboard.

**Grid**
- Show any of about 30 fields: right-click a header or use "Add New Column". You can also hide, move, resize and reset columns, and the layout is saved with the plan.
- Cut, copy and paste tasks (Ctrl+X/C/V). Paste rows straight from Excel.
- Fill down (Ctrl+D), move rows up and down (Alt+Shift+↑/↓), and Find, Find Next and Replace (Ctrl+F, F3, Ctrl+H).
- Right-click menu for tasks. Indicators for manual scheduling, notes and constraints. Inactive tasks are struck through.

**Gantt chart**
- A timeline strip above the Gantt (MS Project style): add tasks with right-click › Add to Timeline, and click it to jump to a date.
- Drag the progress line inside a bar to set % complete. Dragging one bar onto another creates a link.
- Deadline markers, a status date line, manual task bars, and resource names next to the bars.
- A details pane below the Gantt (Primavera style), with tabs for General, Relationships (edit link type and lag, add or remove predecessors and successors), Resources and Notes.

**Everyday use**
- Ctrl+K searches every command, view and task.
- Ctrl+J quick-adds a task in one line: `Pour slab 5d after 4 @Crew[50%] by 20-Nov-26`.
- Options: date format, and a recovery copy of unsaved work every few minutes. If the app closes unexpectedly, it offers to recover your work.
- The status bar switches new tasks between auto and manual scheduling.
- Ribbon tabs: Task, Resource, Project, View and Help.

**Import and export**
- Import Microsoft Project XML and Primavera P6 XER, including calendars, WBS, relationships, resources, actual dates and baselines.
- Import Excel (.xlsx/.xls) or CSV task lists, with a step to match columns. Predecessors can refer to your own IDs, such as A10.
- Export to Excel, CSV, MS Project XML and Primavera XER.

**Fixed**
- Project Information now shows the start date, and dialogs keep what you typed when you press OK.

**Known limits in 0.2**
- No resource leveling yet (0.4.2). Overallocation is shown so you can fix it by hand.
- XER export follows the P6 format but has not been tested in P6 itself. Please try it and report any problem.
- Printing prints the current view. Page setup for the Gantt chart comes later.

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
