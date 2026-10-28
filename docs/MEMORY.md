# Datum Plan: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Plan: a Windows desktop scheduler combining MS Project (look, ease of use) and Primavera P6 (power features) |
| **Current version** | **0.4.1** (released 2026-10-28). Earlier: 0.1.0 … 0.4.0 |
| **Repo** | `Edgar-Mendonca/Datum-Plan` · working branch `claude/fervent-keller-ddas5s` (currently also the default branch) |
| **Stack** | Rust (engine + shell), Tauri 2, WebView2, SolidJS + Vite UI in strict TypeScript (TSX), plain CSS with light/dark tokens, Chart.js, IBM Plex fonts; Excel formulas from the GridCraft crates (pinned git dependency, see NOTICE) |
| **Code size** | About 9,300 lines: core 5,700 (with 460 lines of tests), shell 700, UI 2,900 TSX + CSS |
| **Installer** | NSIS `.exe`, per-user install. 1.7 MB (0.1.0), 1.8 MB (0.1.1), 2.4 MB (0.2.0); UI bundle 200 kB JS (61 kB gzip) in 0.3.0 |
| **Reference repos** | `Edgar-Mendonca/Cairn` (the earlier web planner: feature and template reference) · `Compressor-Engineering-Suite` (the Home/update pattern) |

---

## 1. Architecture

```mermaid
flowchart LR
  subgraph UI["Window (WebView2) · ui/src/*.tsx → Vite → ui/dist"]
    Store[store.ts: signals for plan, view, table, filter,<br/>group, sort, selection; act(op) and refresh()]
    Rib[ribbon.tsx: tabs, dropdowns, QAT, Tell Me]
    Grid[grid.tsx: virtual rows, inline edit,<br/>header menus, column drag/resize]
    Gantt[gantt.tsx: canvas Gantt + timeline]
    Det[details.tsx: P6 tabs]
    Views[views.tsx: network, calendar, usage, resources,<br/>graph, team, risks, checks, portfolio]
    Rep[reports.tsx: dashboard, report gallery, charts]
    Dlg[dialogs.tsx: information, calendars, custom fields,<br/>filters, groups, sorts, baselines, import, compare]
    BS[backstage.tsx + setup.tsx: File, Options, updates,<br/>Project Setup panel]
  end
  subgraph Shell["src-tauri (Rust)"]
    Cmd[main.rs: state · apply · history · query · resources · field_list<br/>task_info · usage · curve · checks · network · compare · workspace<br/>open · save · import · export · save_file · pick_logo · options · recovery]
    Upd[update.rs: GitHub API · beta channel · SHA-256 · silent install]
  end
  subgraph Core["crates/core (pure Rust, 18 tests)"]
    Doc[doc.rs: Op enum · undo/redo · .dplan]
    Sched[sched.rs: CPM · calendars · progress · LOE<br/>longest path · float paths · from finish]
    Level[level.rs: resource leveling]
    Fields[fields.rs: Calc · ~450 fields · EV · custom fields<br/>formulas · indicators · filter tests]
    Query[query.rs: filter · group · sort · outline → Grid + bars]
    Rep2[report.rs · check.rs · network.rs<br/>usage · curves · compare · DCMA · layout]
    Form[formula.rs: MSP formula language]
    Sheet[sheet.rs: GridCraft Excel engine:<br/>formula columns · sheet tabs · Plan sheet]
    Load[load.rs: timephased work and cost]
    IO[io/: mspdi · xer · table · formats pmxml/mpx/ics/html/json]
  end
  UI -- "invoke(cmd, args)" --> Cmd
  Cmd --> Doc & Query & Rep2 & IO
  Doc --> Sched
  Query --> Fields --> Sched & Load & Form & Sheet
  Rep2 --> Fields
```

**Rules:**
- The UI never calculates dates, values or filters. Every edit is an `Op`; the core applies it to a copy, normalises, schedules and keeps it only if valid.
- After an edit `apply` returns `Out` (project settings without tasks + `Summary`). The window then calls `query(Query)` for the rows it shows; `Calc` (fields.rs) evaluates any field for any row.
- Window-only settings (saved filters, extra tables) live in `Project.ui`; the Entry table's columns in `Project.columns`.

### Time model (since 0.2)
- **Instants** are minutes since 1970-01-01 00:00 local time. **Durations and lags** are working minutes.
- `Project.day_min` (default 480) converts days. A week is 5 days and a month is 20 days.
- `Task.unit` remembers how the duration was typed (h, d, w, mo) so it is shown the same way.
- **Calendars:** `week: [Vec<(start_min, end_min)>; 7]` (Monday first) plus `exceptions` (date ranges; empty hours mean not working).
- The project calendar is `Project.calendar`. Each task can use its own `Task.cal`.
- **Scheduling:**
  - Forward pass: `ES = max(link requirements, constraints, data date)`, snapped to working time. Milestones may sit at the end of a shift.
  - Backward pass: works on late dates, which SNLT, FNLT, MSO, MFO and deadlines all limit.
  - Lags count on the successor's calendar.
  - Total float = working time between EF and LF.
  - A link to or from a summary applies to every leaf task under it.
  - `normalize()` drops links between a task and its own ancestors.
- **Progress:** an actual finish fixes the task. An actual start with % < 100 puts the remaining work after the status date (status date + 1 day, 00:00). Unstarted tasks cannot start before the status date. Setting % fills in actual dates; 0% clears them.

### Files

| Path | What it does |
| --- | --- |
| `crates/core/src/model.rs` | File format 3: Task (also task type, effort, activity/% complete type, physical %, elapsed, splits, leveling delay, priority, fixed cost + accrual, custom values, codes, steps, notebook, expenses, docs, 11 baselines in `bls`), Resource (rate tables, availability, calendar, overtime), Project (setup fields, logo, currency, Activity ID numbering, `SchedOpts`, custom field defs, codes, risks, snapshots, `ui`). Migrates formats 1–2. |
| `crates/core/src/cal.rs` | `WorkCal`: working-time arithmetic. |
| `crates/core/src/sched.rs` | CPM → `Sched` (es/ef/ls/lf/tf/ff, edges with driving flags, longest path, float paths, calendars incl. a 24-hour one). `critical()` follows the options. |
| `crates/core/src/level.rs` | Serial leveling with a fixed rank; `clear`. |
| `crates/core/src/load.rs` | Work/cost per task, assignment and day (rate tables, accruals, actuals); `buckets(day/week/month/quarter)`. |
| `crates/core/src/formula.rs` | MS Project formula parser and evaluator (34 functions). |
| `crates/core/src/fields.rs` | Field registry (`task_fields`, `resource_fields`), `Calc` (one scheduled plan + loads + EV), `val/text/format/icon`, filter `test`, `money`. |
| `crates/core/src/query.rs` | `Query` → `Grid` (cols, rows with cells, icons, bar geometry), built-in filters, `Summary`. |
| `crates/core/src/report.rs` | Usage grids, cost/EV curves, compare; reporting periods (`period_ends`), weighted progress S-curves (`scurve`: planned from baseline, actual linear from actual start to today's %, forecast), `period_report` (this vs previous period, WBS, lists, hours). |
| `crates/core/src/sheet.rs` | GridCraft bridge: value conversion (dates ↔ Excel serials, durations in days), `[Field]` translation, `task_formulas` (all Excel formula columns in one in-memory workbook, one row per task), `book`/`sheet_view` (Plan sheet + user tabs). |
| `crates/core/src/check.rs` | DCMA 14 + P6 checks with limits. |
| `crates/core/src/network.rs` | Network Diagram layout (columns by logic, rows by predecessor position). |
| `crates/core/src/parse.rs` | Durations (also elapsed), dates, predecessors (also % lags), resources, quick add. |
| `crates/core/src/doc.rs` | Ops (see the enum: edits, Level, Split, Interim, UpdateProject, Snapshot, ProjectSet, FieldDef, CodeDef, Risk, TaskList, ResJson, Ui, Baseline n, …), undo/redo, `.dplan`. |
| `crates/core/src/io/` | `mspdi.rs`, `xer.rs`, `table.rs` (CSV/XLSX in with mapping; out with chosen columns), `formats.rs` (JSON, HTML, ICS, MPX, PMXML out), `xml.rs`. JSON/dplan import. |
| `crates/core/examples/` | `bench.rs` (20k-task timing), `dump.rs` (JSON fixtures for UI tests). |
| `crates/core/src/tests.rs` | Engine tests (26). |
| `crates/core/src/risk.rs` | Schedule risk (Monte Carlo): `SimOpts` (iterations, default range −low/+high %, seed), `simulate` → P10/50/80/90, chance of the planned finish, finish-day histogram, cost P50/P80, criticality, drivers (correlation), risk hit rates. Triangular draws; every run is a full `schedule()`. |
| `crates/core/src/progress.rs` | Progress history: `credit_library`, `manpower` (people per period by trade: planned / done / forecast), `earned_schedule` (ES, SPI(t), SV(t), IEAC(t)), `lookahead` (tasks in the next weeks + open/overdue constraints). |
| `ui/src/progress.tsx` | Manpower and Look-ahead views, Rules of Credit dialog. |
| `ui/src/risk.tsx` | The Schedule Risk view (`simrisk`). |
| `crates/core/src/cases.rs` | Hand-worked scheduling cases (47 cases, 90+ task dates): rows as "duration \| preds \| field=value" set through `doc::set`, results as "early start, early finish, float". |
| `crates/core/src/parity.rs`, `crates/core/tests/parity/` | Parity files: XML/XER plans with their own dates (`network.xml`, `network.xer`, hand-made); the test reschedules and compares every task to the minute. Drop real exports from other tools in the folder. |
| `crates/core/src/hardening.rs` | Hardening tests: a failed edit changes nothing, 1,000 random plans schedule with sane dates, ~3,000 damaged XML/XER/JSON files never panic, 2,000 tasks reschedule fast (debug < 400 ms; release ~3 ms). |
| `deny.toml` | `cargo deny check`: advisories, licences (MIT-compatible list), sources (crates.io + gridcraft git). |
| `SECURITY.md`, `CONTRIBUTING.md` | How to report a problem, what the app protects; how to build and the checks to run. |
| `src-tauri/src/main.rs` | Commands listed in the diagram; options (date format, autosave, beta, default currency, company, logo, workspace, setup panel); backups of format < 3 files; recovery. |
| `src-tauri/src/update.rs` | Updater; `check(beta)` reads `/releases` when beta is on; versions compare with pre-release tags. |
| `ui/index.html` | Root and the icon sprite (about 65 icons). `ui/public/logo.svg`. |
| `ui/src/api.ts` | Types matching the Rust JSON, `invoke`, date and money helpers. |
| `ui/src/store.ts` | Global signals, built-in tables, `query()`, `refresh()`, `act()`, menus, dialogs, toast, theme (`applyTheme`, `themeTick`). |
| `ui/src/cmds.ts` | Command functions `C` and the `ALL_CMDS` list for Tell Me. |
| `ui/src/ui.tsx` | Icon, Menu (with submenus), Dialog (optional `help` key puts a ? in its header), Tabs, form helpers. |
| `ui/src/help.tsx` | "?" help: the `HELP` table (`{ t, what, how?, tip? }` per topic), `<Help k>` (the small ? button, `.help`), `helpCard(k)` (the card dialog, `.helpcard`). Ribbon groups take `h=` on `G`, pages `help=` on `Page`/`Tools`. Help › What Is This Tool? lists every topic. |
| `ui/src/grid.tsx` | Excel-like task sheet: one scroller with sticky header and frozen ID/name, ranges (`ranges` signal), fill handle, TSV clipboard via copy/paste events, `Cells` op for range edits, AutoFilter menus, header rename (`Title` op), `FormulaBar`. |
| `ui/src/charts.tsx` | Chart.js wrapper in a Prism style (Okabe-Ito colours, crosshair, status marker, PNG save). |
| `ui/src/sheets.tsx` | Sheet tabs (A1 grid, formula bar, formats, clipboard). |
| `ui/src/print.tsx` | File › Print (every view; `pkind()` = gantt / net / cal / table / flow; `Frame` draws paper + header/footer/legend; `vdata`/`vpages`/`ViewPage` for network tiles, calendar months and tables; `FlowDoc` prints report pages in a table whose thead/tfoot repeat, with page numbers from `@page` margin boxes): `PageSetup` (saved in `project.ui.page`), `layout()` makes one strip (table columns + chart) and cuts it into page windows (across then down), `Strip` draws a window, `Sheet` draws one page as SVG (table + Gantt + legend) with header/footer codes and logos; `printNow` renders `#printroot` and sets `@page` size, then `window.print()`. `PAPERS`. |
| `ui/src/roadmap.tsx` | File › Roadmap: the `ROADMAP` list (keep it in step with the roadmap table and "Pending and next" in this file). |
| `ui/src/selection.ts` | The grid engine's selection, shared by the task sheet and Sheets: `createSel` (ranges, active cell, pick/drag/rows/cols/all/each), `aggregate` (Sum/Average/Count). |
| `ui/src/cells.ts` | Shared by both grids: `parseTsv`/`toTsv`/`transpose`, `series` (Excel fill rules), `clip` (last copy: values + styles for Paste Special). |
| `ui/src/fmt.ts` | Text formatting: style strings `b;i;u;s;c:#hex;f:#hex;z:12;a:c`, `css()`, the focused grid registers a `Target` (get/set selected cells), toggle/setProp/clear, Format Painter. |
| `ui/src/tools.tsx` | Update Progress page, Relationship Diagram (SVG), Report Builder (sections, `TEMPLATES`, saved in `ui.reports`). |
| `tests_ui/run.cjs` | Playwright UI tests against the built UI with a mocked shell and `dump` fixture (alignment, ranges, menus, ribbon, sidebar, every view). Runs in CI. |
| `gantt.tsx`, `details.tsx`, `views.tsx`, `reports.tsx`, `dialogs.tsx`, `ribbon.tsx`, `backstage.tsx`, `setup.tsx`, `main.tsx` | As in the diagram. `styles.css` holds the light theme and print styles. |

### Update flow

```mermaid
sequenceDiagram
  participant U as User
  participant App as Datum Plan (old)
  participant GH as GitHub Releases
  participant Inst as setup.exe
  App->>GH: GET /repos/.../releases/latest (on start + "Check for updates")
  GH-->>App: tag, notes (CHANGELOG section), assets
  App->>U: badge "Update available" + Home card with notes
  U->>App: Update now
  App->>GH: download DatumPlan-X.Y.Z-setup.exe (progress bar)
  App->>GH: SHA256SUMS.txt → verify hash
  App->>Inst: cmd: wait 2s, run /S (silent), then start the app
  App->>App: exit
  Inst-->>U: new version opens
```

### Release flow

```mermaid
flowchart LR
  A[Bump version<br/>src-tauri + core Cargo.toml] --> B[CHANGELOG section<br/>## X.Y.Z — date]
  B --> C[Update docs/MEMORY.md]
  C --> D[Commit + push]
  D --> E[Run release.yml<br/>tag push, or workflow_dispatch<br/>when tags are blocked]
  E --> F[GitHub Release<br/>setup.exe + SHA256SUMS.txt]
  F --> G[Users click Update now]
```

### How we work (the workflow, since session 1)
1. **The user reviews the last release** in the app and sends screenshots with bugs and wishes.
2. **Diagnose first.** Reproduce the problem in the test harness or the real app before changing code. Small fixes go straight to a release.
3. **Bigger asks get a plan file** (`docs/PLAN-x.y.z.md`): feedback, diagnosis, proposal by area, size, and questions. The plan is logged in this file and pushed. Build only after the user answers.
4. **Build in small steps**, engine first (`crates/core`, with a unit test), then the UI. Keep code compact and in the surrounding style.
5. **Check in the real app** under Xvfb (`scratchpad/app.sh` restarts Xvfb and the app, opens the Engineering template and skips Setup), with screenshots of each change. Fix what looks wrong.
6. **Add a UI test** for every reported bug and new feature (`tests_ui/run.cjs`, Playwright against `ui/dist` with the `dump` fixture). Make sure it fails on the old code where possible.
7. **Run the checks:** clippy `-D warnings`, `cargo test --workspace`, `npm run check:ui`, and the UI tests after `npm run build:ui` + `cargo run -p datum-core --example dump > tests_ui/fixture.json`.
8. **Release:** bump both Cargo.toml files, write the CHANGELOG (it becomes the in-app "What's new"), update this file (version, roadmap, files, decisions, session log, limits), commit, push, run `release.yml` (workflow_dispatch), then watch CI + Release and confirm the installer.
9. **Report to the user** in plain language: what changed, what was checked, what is not done, and anything to check on Windows.

Releases so far: 0.1.0, 0.1.1, 0.2.0, 0.3.0 (MSP look + P6 power), 0.3.1 (Excel grid, formulas, periods), 0.3.2 (builder, power tools), 0.3.3 (hotfix), 0.3.4 (print + formatting), 0.3.5 (ribbon tabs, Setup page, columns), 0.3.6 (print every view, network, timeline zoom), 0.3.7 (tracking, templates, whiteboard network, grid engine), 0.3.8 (print links, date picker), 0.3.9 (? help beside the tools), 0.3.10 (Roadmap), 0.3.11 (hardening quick wins), 0.3.12 (trust in the dates), 0.4.0 (schedule risk, trades, levelling preview, import report), 0.4.1 (progress history).

---

## 2. Decisions

| Date | Decision | Why |
| --- | --- | --- |
| 2026-10-24 | Name **Datum Plan**, file extension `.dplan` | The user chose it. |
| 2026-10-24 | **Rust engine** in `crates/core`; Cairn is only a reference | The user wants everything in Rust for a small, robust app. |
| 2026-10-24 | UI in plain TypeScript with no framework, split into 7 script files joined by tsc | Small bundle; modules without a bundler. |
| 2026-10-24 | Custom updater (GitHub API + SHA-256 + NSIS `/S`); per-user install | Needs no signing secrets; silent updates. |
| 2026-10-24 | Windows only; no `.mpp` (closed format) | The user's choice. Exchange formats are dplan, XML, XER, XLSX and CSV. |
| 0.1.1 | Light theme, new logo, Cairn-style sidebar, general templates | The user's feedback on 0.1.0. |
| 0.2.0 | **Minute-precision engine with real calendars** (replacing whole days) | Needed for hours, shifts, holidays and P6/MSP exchange. Old files migrate (day × 480). |
| 0.2.0 | Several roadmap steps shipped as one release (0.1.2–0.1.4, calendars, constraints, logic, exchange, resources, baselines/progress) | The user asked for all of it in 0.2. |
| 0.2.0 | Links on summaries apply to every leaf under them; links to one's own summary are dropped | Simple and close to MSP. P6 has no summary links anyway. |
| 0.2.0 | Status date semantics: progress up to the end of that day; remaining work starts the next day | MSP-like status date. XER `last_recalc_date` maps to status = data date − 1. |
| 0.2.0 | Work-resource units: `50%`, `0.5` or `50`; material: quantity; cost: amount | Matches MSP habits. |
| 0.2.0 | Predecessor IDs typed with new rows (quick add, paste) mean the IDs shown before the insert | What the user sees on screen. |
| 0.2.0 | XML reader written by hand instead of quick-xml | The quick-xml API changes a lot between versions; MSPDI only needs elements and text. |
| 0.2.0 | Recovery copy in AppData, not autosave over the user's file | Safe: never overwrites a file without asking. |
| 0.3 plan | **SolidJS + Vite** for the UI (not Astro) | Small reactive components for MSP-level dialogs, ribbon and report designer. Astro targets content sites. |
| 0.3 plan | **Local-first:** `.dplan` files plus a workspace file; no server, no cloud, no telemetry (the only network call is the update check) | Open source, everything stays on the user's computer. |
| 0.3 plan | Betas through an opt-in updater channel; slipped items go to 0.3.1 | The user wants to check the big release step by step. |
| 0.3 plan | **Datum scheduling style** (PLAN-0.3 §3.P): fixed duration, single Progress field, retained logic, slack ≤ 0 critical with a longest-path switch, auto actuals, an Explain panel, calculated cells marked | The user wants something easier than both MSP and P6. |
| 0.3 plan | **Project Setup** panel (skippable, docked right) as step one of every new plan; all ISO currencies; logo and company branding; Activity IDs A1000+10; MSP-style network; MSP density; English only; beta backups | The user's answers, 2026-10-24. |
| 0.3 plan | `docs/reference/MSP-P6-feature-reference.md` (from the user) is the behaviour spec; its C2 scenarios become tests and a checklist | A shared, written definition of done. |
| 0.3.0 | Shipped as one release (no betas) because the user asked to "build and release 0.3" | Betas stay available through the opt-in channel for later versions. |
| 0.3.0 | The engine owns fields, filters, groups and sorts (`Calc` + `query`); the window asks for rows per view | One place for values, exports use the same text; 20k tasks query in ~50 ms. |
| 0.3.0 | Print/PDF through the WebView print dialog ("Save as PDF") with print CSS, instead of a PDF library | No extra dependency; works for every view and report. |
| 0.3.0 | Templates still use the Project Setup panel (setup_done = false) | Setup is step one of every new plan, as agreed. |
| 0.3.1 | Keep the MS Project ribbon and layout; adopt only CentriForge's styling (IBM Plex, palette, square controls, mono labels) | The user's answer: improve the current UI, don't replace the command bar. |
| 0.3.1 | Light / Dark / Auto theme via CSS tokens on `:root[data-theme]`; canvas colours read the tokens | The user asked for all three. |
| 0.3.1 | Formulas are per column (MS Project style) in Excel syntax with `[Field]` names, evaluated by GridCraft; cell references only in sheet tabs | The user's answer. GridCraft gives ~400 Excel functions without writing them. |
| 0.3.1 | GridCraft as a pinned git dependency (calc, model, core), not a fork | Small surface (one module), easy upgrades, attribution in NOTICE. |
| 0.3.1 | Grid stays DOM (virtual rows) with a single scroller and sticky header, not canvas | Fixes the alignment bug at the root; real inputs for editing. |
| 0.3.1 | Reporting periods per plan (week end day, month close day, weighting) chosen by the user | The user's answer: it varies by project. |
| 0.3.1 | Charts with Chart.js in a Prism style; PNG save on every chart | The user asked for elegant, hoverable charts like Prism/plotly; Chart.js is small. |
| 0.3.1 | Period notes and photos stored inside the .dplan (photos downscaled to 1400 px JPEG) | Portable single file; the user left it to us. |
| 0.3.2 | Big ribbon buttons with both a command and a menu are MS Project split buttons; contextual tabs carry their group label above the tab | The user's screenshots of 0.3.1. |
| 0.3.2 | Report Builder reports and conditional-format rules live in `project.ui` (no format change) | Saved with the plan, no migration. |
| 0.3.2 | Recurring tasks are a summary with SNET-constrained occurrences | Same as MS Project; occurrences stay ordinary tasks. |
| 0.3.3 | Effects that mirror outside selection into the grid use `on(sel, …)` with `untrack`, never track `cur`/`ranges` | Tracking `cur` made the effect run before the sel sync and snap the row back (the 0.3.3 bug). |
| 0.3.3 | Task sheet and Sheets tabs should share one grid component (selection, clipboard, fill, formula bar); GridCraft stays the formula engine underneath; scheduling stays in `crates/core` | The user asked to use the Sheets grid for scheduling. Task rows are not free cells (they carry types, links, outline, validation), so the shared part is the UI grid, not the data model. |
| 0.3.9 | **? help beside the tools in every Datum app**: a small ? beside each ribbon group, page, panel and main dialog opens a card (what it is for, how to use it) from one `HELP` table in `ui/src/help.tsx`; Help › What Is This Tool? lists them all. No separate teaching mode | The user's words: "at some of the tools, we can add a small question mark icon… we should do it in all of our applications." Built first in Datum Atlas 0.1.1; same code and styles in every app. |
| 0.4.1 | Actual progress between stored periods is a straight line through each snapshot's % (then to today's % at the status date); tasks no snapshot recorded keep the old working-time line (`report::history`, `along`) | The actual S-curve should show when progress really happened; snapshots already stored each task's %. |
| 0.4.1 | Manpower "Done" is earned hours (planned hours × progress), not booked hours; planned spreads work evenly over baseline (or current) dates; people = hours ÷ one person's working hours in the period | There are no timesheets in the model; earned hours are honest and consistent with the S-curve. |
| 0.4.1 | Earned schedule uses the same weights and curves as the S-curve, in working days on the project calendar | One definition of progress everywhere; works without costs. |
| 0.4.1 | Rules of credit are step templates (`CreditRule`) that `ApplyCredit` copies into a task's steps and sets % Complete Type to Physical; quantities set physical % = installed ÷ planned. Physical % now drives the S-curve for physical tasks | Reuses steps and physical %, which already drove earned value. |
| 0.4.1 | Constraint log is `Project.log` (`LogItem`), edited with `Op::Log`; ready = no open items; overdue = open with a need-by date before the status date | Look-ahead planning needs readiness, not just dates. |
| 0.4.1 | `<select>` whose options load later marks the chosen `<option selected>` instead of `value=` on the select | Solid sets the select's value before async options exist, so it showed the first option. |
| 0.4.0 | Monte Carlo uses triangular draws (min, planned, max) and the full scheduler per run; a fixed seed (same plan + settings = same answer); risks 1–5 → 10/30/50/70/90 %; `Risk.days` = working days added to each linked task (opportunities subtract); cost scales with duration except fixed cost | Simple, explainable, and exact to the engine's own rules (calendars, constraints, progress). |
| 0.4.0 | Trades are the resources' Group; `usage("group")` sums work resources per group with capacity; rows carry `cap`, `Usage.person` = one person's hours per bucket (hours → people) | No new model field; the window shows the engine's capacity instead of computing it. |
| 0.4.0 | Levelling always previews (`level::preview` levels a clone and diffs the dates) | See what moves before changing the plan. |
| 0.4.0 | Imports return notes (`Imported::Plan(p, notes)`): reader notes plus a date comparison with the file (`io::stated_dates`, shared with the parity test) | The import report the user asked for in 0.3.12; the comparison makes every import a parity check. |
| 0.4.0 | Charts over plain numbers use `<Index>`, not `<For>` | `<For>` reuses nodes by value, so bars with equal values kept stale positions. |
| 0.3.12 | ALAP uses free float only: the largest shift (≤ free float) that every successor link still allows, found by bisection with the forward pass's own rule | Standard rule; using total float pushed ALAP tasks past their successors (found by the random-plan link check). |
| 0.3.12 | A started task's late start is that of its remaining work; finished successors (and, with progress override, started ones) do not limit a predecessor's late dates | Standard progress rules; the old backward pass gave false negative float under retained logic (found by a hand-worked case). |
| 0.3.12 | Parity files are hand-made until real exports are added; the README says "checked against hand-worked cases and plan files", never names another product | Honest claim; FAMILY §13. |
| 0.3.11 | `docs/site.json` carries `about` (two paragraphs) and `steps` (three) for the family site's app page; keep them true to the app and update them with every release; never name commercial products | The user, for the hub's `/apps/plan` page (FAMILY §10, §13). |
| 0.3.11 | rustfmt is not enforced; the code stays in its compact hand style | `cargo fmt` would add about 3,000 lines to ~7,400 and undo the compact style the user asked for. Clippy `-D warnings` stays the gate. |
| 0.3.11 | Hardening gates in CI: `cargo deny`, Windows + macOS `cargo test`, hardening tests in `cargo test` | Match the rigour of other open-source schedulers before adding features. |
| 0.3.11 | Without a status date, progress reports (Dashboard, period reports, S-curves) measure at max(today, latest recorded progress) (`Calc::progress_date`/`progress_at`), and the window says so (`Summary.measured_at`). Earned value and the fields still use the status date or today (`status_at`) | A plan updated ahead of time showed 0 % done. |
| 0.3.11 | The window's state lock recovers from a poisoned mutex (`St::lock`) | One panicking command used to lock every later command out. |
| 0.3.8 | The Datum apps share one design and workflow in [FAMILY.md](FAMILY.md); each app only changes its accent and logo glyph | The user is building a family of tools (next: Datum Plot = Prism + OriginPro) that must look and ship the same. |
| 0.3.7 | Templates carry `people`/`assign`/`costs`; Work is titled "Man-hours" | New users see loading, cost and man-hours immediately (the user's Cairn did this). |
| 0.3.7 | One grid *engine* (selection, clipboard, fill, formatting, keys) with two renderers (task outline vs A1 sheet) | The views differ in what a row is; sharing the behaviour removes the duplication without forcing one layout on both. |
| 0.3.7 | Network: `layout_with(rows, sums, wbs)`; bands returned with the layout | WBS banding and summary boxes come from the engine, so screen and print agree. |
| 0.3.6 | Other views print through one `Frame`; reports print as flowing documents (thead/tfoot repeat the header/footer, `@page @bottom-center` numbers pages) | Exact pages where the layout is ours (Gantt, network, calendar, tables); flowing pages where it is HTML. |
| 0.3.6 | Fill series follow Excel (numbers, text+number, dates by step; Ctrl copies) but durations, links and resources copy as in MS Project | Matches what users of both tools expect. |
| 0.3.6 | `open_url` only opens github.com/Edgar-Mendonca/Datum-Plan links | The window cannot be used to open arbitrary URLs. |
| 0.3.5 | Ribbon tabs: Task, Schedule, Progress (user rejected "Home"); the ribbon shrinks in steps (`data-fit` 0–3) and never scrolls | User feedback: crowded Task tab and a sideways scrollbar. |
| 0.3.5 | Hidden columns stay in the table with a negative width (`Project.columns` is now `i32`); `cols()` = shown, `allCols()` = all | Hide/unhide without losing the position; files stay compatible. |
| 0.3.5 | Print lays the table and chart out as one strip; pages are windows on it; ID + name repeat; column edges snap | Every column prints, like MS Project, with no overlap. |
| 0.3.4 | Printing lays out pages in the UI (SVG) and uses the WebView print dialog (Save as PDF), not a Rust PDF writer | Vector output, exact page sizes via `@page`, no new dependency; works the same on every view we lay out. |
| 0.3.4 | Cell formatting reuses the task's existing `style` JSON field (column key → style string); sheets get `SheetData.style` | No file-format change; old files load unchanged. |
| 0.3.3 | Every user-reported bug gets a UI test that fails on the broken build before the fix ships | Two clicks in a row were never tested; the 0.3.2 suite missed it. |
| 0.3.1 | No Python; a separate future app "Datum Plot" (Prism/Origin-like) may use Python | The user's answer. |

---

## 3. Roadmap

✅ done · 🔜 next · ⬜ planned. Small releases; every one goes through the in-app updater.

| Version | Theme | Contents | Status |
| --- | --- | --- | --- |
| **0.1.0** | Shell + updater + core | Installer, Home, updater, ribbon, grid, summaries, links, CPM, Gantt, undo, `.dplan` | ✅ |
| **0.1.1** | Look, views, reports | Light theme, logo, sidebar, templates, drag bars, Calendar, Dashboard, weekly/monthly report | ✅ |
| **0.2.0** | The big one | Grid power (any column, clipboard, fill, move, find/replace, menus), Gantt power (timeline, progress drag, details pane, link editing), Ctrl+K, Ctrl+J, Options, recovery, minute calendars + exceptions + task calendars, all constraints + deadlines, manual/inactive tasks, summary links, status date + actuals + Mark on Track, baselines + variance, resources + costs + workload, import/export XML/XER/XLSX/CSV | ✅ |
| **0.3.0** | MSP look + P6 power (one big release) | Everything planned for 0.2.1 → 0.5.2, network diagram included. Full scope and beta milestones in **[PLAN-0.3.md](PLAN-0.3.md)**: MSP ribbon with dropdowns and Format tabs, about 450 fields + custom fields/formulas/lookups/indicators, Field Settings, Information dialogs, tables, filters, groups and views, formatting, task types, leveling, P6 activity types, longest path, usage views, Team Planner, Network Diagram, 11 baselines, Update Project, earned value, costs, report gallery and designer, page setup/PDF, DCMA check, compare, risks, EPS workspace, new formats (P6 XML, MPX, ICS, HTML, JSON) | ✅ (see 0.3.1 for what slipped) |
| 0.3.1 | Foundation ([PLAN-0.3.1.md](PLAN-0.3.1.md)) | Excel-like grid, GridCraft formula columns, sheet tabs, reporting periods, period progress reports with notes/photos, Prism-style S-curves and dashboard, CentriForge styling, light/dark/auto | ✅ |
| 0.3.2 | Builder & power | Report builder and templates, Update Progress workflow, xlsx export with formulas, recurring tasks, relationship diagram, global change, EPS tree, conditional formatting, Playwright UI tests in CI | ✅ |
| 0.3.3 | Hotfix | Grid row selection stuck after the first click | ✅ |
| 0.3.4 | Print & format | Print page with Page Setup (ranges, paper, scaling, margins, header/footer codes, logos, legend, columns), vector pages; Font/Alignment formatting for task sheet and Sheets; Options page rebuilt | ✅ |
| 0.3.5 | Ribbon, Setup, columns, print ([PLAN-0.3.5.md](PLAN-0.3.5.md)) | Task/Schedule/Progress tabs, ribbon fits without scrolling, Setup page in the sidebar, nested menus, hide/unhide/delete columns + Columns dialog, print every column (strip layout) | ✅ |
| 0.3.6 | Print every view, Network, timeline zoom ([PLAN-0.3.6.md](PLAN-0.3.6.md)) | Font family + fill icon, everyday commands on the Task tab, Check-again feedback + GitHub/Releases/Issues links, Getting Started + Glossary, timeline zoom box, Network Diagram (legend, box styles, zoom, pan, mini-map, path), page layout for every view, xlsx formatting, series fill + Paste Special in both grids, Sheets freeze panes | ✅ |
| 0.3.7 | Tracking, templates, network, one grid | Real Tracking Gantt (baseline bars, Tracking table, banner); templates with people/8h/assignments/man-hours/costs; whiteboard network with summaries and WBS bands; shared grid engine (`selection.ts` + `cells.ts` + `fmt.ts`); icon set | ✅ |
| 0.3.8 | Fixes | Links in printed Gantt charts; date picker on date cells and fields | ✅ |
| 0.3.9 | Help | A ? beside every ribbon group, page, panel and main dialog; Help › What Is This Tool?; OK-less dialogs close with Escape | ✅ |
| 0.3.10 | Roadmap + fixes | File › Roadmap (`ui/src/roadmap.tsx` reading `docs/site.json`); print fit-to-pages fix; lighter network cross | ✅ |
| 0.3.11 | Fixes + hardening quick wins | Dashboard progress without a status date; date picker closes; hardening tests (found and fixed 2 XER crashes); poisoned-lock recovery; `cargo deny`; CI on Windows + macOS; SECURITY/CONTRIBUTING | ✅ |
| 0.3.12 | Trust in the dates | Hand-worked cases (`cases.rs`), parity files (`parity.rs`), random plans with constraints/calendars (links hold, tight forward pass, row order), random edit sequences, damaged CSV/xlsx; fixed ALAP (free float) and late dates of started work | ✅ |
| 0.4.0 | Risk & resources | Schedule Risk (Monte Carlo: P50/P80/P90, cost, criticality, drivers, risk delays, optimistic/pessimistic durations), workload by trade (Group) in hours or people, levelling preview, import report + XER activity types | ✅ |
| 0.4.1 | Progress history | Actual S-curve through the stored periods, Manpower by trade (planned / done / forecast), earned schedule, rules of credit + installed quantities, look-ahead with constraint log | ✅ |
| 0.4.2 | Command line & automation | Hardening plan items 10–12: `datum-cli`, named commands with schemas, MCP server, exact money | 🔜 |
| 0.6.0 | Portfolio | Portfolio dashboard, master/subprojects, shared resource pool, cross-project links | ⬜ |
| 0.6.1 | Hardening | 100k-activity performance, code signing, signed updater, crash reports, dark theme | ⬜ |
| 1.0.0 | Stable | — | ⬜ |

### Features that go beyond MSP and P6
- Weekly/monthly progress reports ✅
- Dashboard with S-curve and baseline curve ✅
- Command search ✅ and quick-add sentences ✅
- Recovery copies ✅
- Project Setup panel, DCMA check with drill-down, risk register, snapshots with trend ✅ (0.3.0)
- Later: schedule health score, one-click report pack, what-if comparison

### Gap analysis (MSP, P6 and Cairn vs Datum Plan)
| Area | MS Project | Primavera P6 | Cairn 0.8 | Datum Plan |
| --- | --- | --- | --- | --- |
| Grid, outline, links, CPM | ✓ | ✓ | ✓ | ✓ 0.1 |
| Interactive Gantt, timeline | ✓ | partial | ✓ | ✓ 0.2 |
| Column picker | ✓ | ✓ | ✓ | ✓ 0.2 (custom fields and formulas in 0.3) |
| Calendars (hours, exceptions, many) | ✓ | ✓ | ✓ | ✓ 0.2 (resource calendars in 0.3) |
| Constraints, deadlines, manual, inactive | ✓ | ✓ | ✓ | ✓ 0.2 |
| Task types, effort-driven, recurring, split | ✓ | duration types | ✓ | ✓ 0.3 (recurring in 0.3.1) |
| File exchange (XLSX, CSV, XML, XER) | ✓ | ✓ | ✓ | ✓ 0.2 |
| Resources, costs, workload | ✓ | ✓ | ✓ | ✓ 0.2 (leveling in 0.3, rates over time in 0.3) |
| Baseline, actuals, status date | ✓ | ✓ | ✓ | ✓ 0.2 (11 baselines and earned value in 0.3) |
| EPS, activity codes, multi-project | master projects | ✓ | — | ✓ 0.3 codes + portfolio folder |
| Filters, groups, layouts, global change | ✓ | ✓ | ✓ | ✓ 0.3 (global change 0.3.1) |
| Schedule check, longest path, float paths | — | ✓ | partial | ✓ 0.3 |
| Network diagram | ✓ | ✓ | ✓ | ✓ 0.3 |
| Dashboards and periodic reports | basic | basic | 16 reports | ✓ 0.3 (10 reports + dashboard) |

---

## 4. Session log

### 2026-10-24: Session 1 (brainstorm + 0.1.0)
- Read Cairn and CentriForge. Wrote the brainstorm and a draft logo.
- The user approved the name, Windows only, no `.mpp`, Rust/Tauri 2, and small releases. They sent MS Project screenshots.
- Built and released 0.1.0: Rust core, Tauri shell, custom updater, TypeScript UI, CI and release workflows.

### 2026-10-24: Session 2 (0.1.1)
- Feedback: too dark, logo too heavy, no domain sample, Gantt not interactive, wanted a Cairn sidebar and periodic reporting.
- Released 0.1.1: light theme, new logo, sidebar, templates, drag bars, Calendar, Dashboard, Reports. Rewrote the roadmap.

### 2026-10-24: Session 3 (0.2.0)
- The user asked for 0.1.2, 0.1.3, 0.1.4 and the "0.2 onwards" items (calendars, constraints, exchange, resources) all in 0.2. They shared a "P6 planning basics" chart (EPS/WBS/activities/calendars/relationships/F9/critical path/resource loading/baseline/update progress).
- Rewrote the engine on minutes with real calendars. Added constraints, deadlines, ALAP, manual and inactive tasks, summary links, status date and actuals, resources and costs, workload, and baselines.
- Added IO: MSPDI, XER, XLSX/CSV, all with round-trip tests.
- Split the UI into 7 files. Added the column picker, clipboard, fill, move, find/replace, context menus, timeline strip, progress drag, details pane, Resource Sheet, Workload, Change Working Time, Options, recovery, Ctrl+K and Ctrl+J.
- Bugs fixed during testing:
  - dialogs lost their inputs on OK (also present in 0.1.x)
  - camelCase fields were missing in the view JSON
  - the details pane overlapped the Gantt canvas (fixed with a ResizeObserver)
  - pressing Enter in Ctrl+K also clicked the dialog it opened
  - quick-add IDs were shifted by the new row
  - CSV header guessing put two columns on the same field
  - alphanumeric predecessor IDs were dropped
- Checked by running the real app under Xvfb: Gantt, right-click menu, timeline, details (General/Relationships/Resources), Resource Sheet, Workload, palette, Change Working Time (Saturday working moved the finish from 08-Feb to 21-Jan), CSV import with mapping, baseline bars, Dashboard. 13 core tests and clippy `-D warnings` pass.
- Released 0.2.0.

### 2026-10-24: Session 4 (0.3 brainstorm)
- Feedback on 0.2.0:
  - the UI does not feel like MSP/P6
  - too few fields, and no custom fields, formulas or Field Settings
  - weak information dialogs and reporting
  - ribbon buttons such as "Gantt Chart" should be dropdowns
  - wants MSP's timeline and more save formats
- Shared MSP screenshots: the column header menu, the Custom Fields dialog, the Task ribbon and the timeline.
- Agreed that 0.2.1 → 0.5.2 become a single 0.3 release. Wrote [PLAN-0.3.md](PLAN-0.3.md) (audit, principles, feature catalogue A–O, architecture changes, beta milestones, open decisions). Waiting for the user's review and decisions before building.
- The user answered all questions and supplied `docs/reference/MSP-P6-feature-reference.md`. Plan updated with the Datum scheduling style, the Project Setup workflow, currency, branding, capacity and behaviour rules (§3.P–T).

---

### 2026-10-24: Session 5 (0.3.0)
- The user said "build and release 0.3". Built it as one release.
- Engine: model format 3; scheduler upgrades (elapsed, % lags, splits, LOE/WBS summary, retained logic/progress override, schedule from finish, longest path, float paths, lag calendars); leveling; task types; formula language; field registry with EV; query API; usage/curves/compare; DCMA + P6 checks; network layout; new export formats. Benchmark: 20k tasks schedule 36 ms, calc 44 ms, grid 49 ms (release).
- Shell: new commands, beta channel, backups of older plans, logo picker, file saving for pictures and HTML.
- UI: rewritten in SolidJS + Vite (10 TSX files). Ribbon with dropdowns, Tell Me, grid with header menus, Gantt + timeline, P6 details tabs, all views, reports, dialogs, Project Setup panel.
- Tested in the real app under Xvfb: templates, Project Setup, inline edits rescheduling, context menu, Task Information, baseline + progress, Tracking Gantt, weekly report, Network Diagram, Calendar, Task Usage, Checks, Dashboard. Bugs fixed: a global `svg` size rule hid the network diagram; Enter in a dialog's first select did not press OK; select text clipped in WebKitGTK; keys only worked with focus inside the app div.
- UI test harness: `cargo run -p datum-core --example dump > fix.json`, then a Playwright script that serves `ui/dist` with a mocked `window.__TAURI__` (kept in the session scratchpad; rebuild it from this note if needed).

### 2026-10-24: Session 6 (0.3.1 brainstorm)
- The user reviewed 0.3.0: buggy, misaligned cells, no range selection, no obvious formulas (e.g. % complete − baseline %), weak reporting (wants dashboard → charts → tables → notes, week vs previous week with variance), UI not up to CentriForge's standard.
- References given: `storytold/gridcraft` (pure Rust Excel engine, MIT/Apache; cloned read-only), `Edgar-Mendonca/Compressor-Engineering-Suite` (design to copy: IBM Plex, square, mono labels, Launcher/Workspace), Cairn. The user suggested Python libraries.
- Wrote [PLAN-0.3.1.md](PLAN-0.3.1.md): diagnosis, Gridcraft crates as pinned dependencies, canvas spreadsheet grid, CentriForge shell and modules, period reporting, Python recommendation (none in 0.3.x), release steps 0.3.1–0.3.3, 10 questions. Waiting for answers and the user's bug write-up.

### 2026-10-24: Session 6, continued (0.3.1)
- Answers: keep the ribbon (style only), light/dark/auto, Excel formulas per column with field names, sheet tabs OK, no Python (future app Datum Plot), user-chosen periods, all kinds of S-curves (elegant, hoverable, printable), notes/photos our call, build it all as 0.3.1. Also: rename headings by double-click, rename tasks from the right-click menu, multi-cell selection like Excel. Screenshot showed misaligned headings.
- Engine: `sheet.rs` (GridCraft), `Cells`/`Title`/`Sheet`/`SheetCells`/`Note` ops, model `titles`, `sheets`, `periods`, `notes`; fields Baseline % Complete and % Complete Variance; periods, S-curves and period reports in `report.rs`; 19 tests.
- UI: grid rewrite, formula bar, Formula Column dialog, Sheets view, `charts.tsx`, Dashboard/Progress/S-Curves pages, period settings, theme switch, CentriForge styling.
- Bugs found while testing: context-menu items never ran (document mousedown closed the menu before click; Solid's delegated stopPropagation does not stop native listeners); a selection-sync effect reset ranges to the old column (fixed with `batch` + `untrack`); cost/hours S-curves showed duration-weighted values for plans without costs (now hidden, `SCurve.weight`).
- Checked in the real app (Xvfb): grid editing, range drag, formula column `=[% Complete]-[Baseline % Complete]`, Progress report, S-curves, Dashboard, Sheets with `=SUM(Plan!F2:F30)`, dark theme.

### 2026-10-24: Session 6, continued (0.3.2)
- User feedback on 0.3.1: dropdown arrows under big buttons, no icons in the heading menu, Format tab and Gantt Chart Tools label squeezed, Gantt Chart Style gallery missing, File sidebar black. All fixed.
- Engine: `Recurring` op, xlsx export with typed cells, live formulas, sheet tabs and a Plan sheet; EPS folders in `workspace`; 20 tests.
- UI: `tools.tsx` (Update Progress, Relationship Diagram, Report Builder), Global Change, conditional formatting, bar styles, gridlines, split buttons, menu icons.
- Tests: `tests_ui/run.cjs` in CI (Playwright). Caught a Sheets crash (ResizeObserver on a missing element) and a CSS class clash (`upd`).

### 2026-10-24: Session 6, continued (0.3.3 hotfix)
- User: could not click or select cells (row stuck on the first selection). Cause: in `grid.tsx` the effect that follows Gantt/find selection tracked `cur`, ran before the effect syncing `sel` from ranges, saw the old `sel` and reset the row. Fixed with `on(sel)` + `untrack`. New UI test "clicking cells selects each one" + "double-click edits a cell" (fails on 0.3.2, passes now). Checked in the real app, including Gantt bar clicks.
- User asked why not use the Sheets (GridCraft) grid for the schedule too. Answer: GridCraft is a calculation engine, not a UI grid; the Sheets view and the task sheet are both our own DOM grids. Plan: one shared grid component for both (0.4.x), keeping the scheduler's task rows. See decision log.

### 2026-10-24: Session 6, continued (0.3.4)
- User asked to merge 0.3.4 and 0.3.5 and fix: printing (incomplete pages, no page control, no date range, no headers/footers/logos/page numbers), the jumbled Options page (stray `<span />` after each `<h4>` shifted the label/control grid), and missing Excel text formatting.
- Built `print.tsx`, `fmt.ts`, `Style`/`SheetStyle` ops, new Options fields (openLast, startView, manualDefault, weekEnd, monthEnd, paper, landscape). Engine test 21. UI tests: Options alignment, Ctrl+B, print preview and print pages with `@page` size.
- Checked in the real app: formatting in task sheet and Sheets, Print page preview (fit to 1 page wide), Options.
- Not done from the planned 0.3.4 (one grid): moved to 0.3.5.

### 2026-10-24: Session 6, continued (0.3.5 feedback)
- User feedback on 0.3.4 (5 screenshots): Project Setup panel too big, move to the sidebar as first icon; ribbon scrolls sideways and the Task tab is crowded (captions cut, poor icons), split into more tabs (need not copy MSP); no Delete Column and no way to unhide; print shows only Name, Duration and a cut-off Start, wants all columns like MSP and the 0.3.4 limits done.
- Diagnosis and proposal in [PLAN-0.3.5.md](PLAN-0.3.5.md); questions sent; build after the answers.

### 2026-10-24: Session 6, continued (0.3.5)
- Answers: keep "Task" (no Home), add more tabs; Setup as a page; hide + unhide + delete columns; printing as proposed; other views' printing, the shared grid and xlsx formatting in 0.3.6.
- Also fixed: the Insert Column categories did not open (`Menu` supported one submenu level; now any depth).
- Built: ribbon tabs and fitting, Setup view, `DeleteField` op, hide/unhide/delete and Columns dialog, print strip layout with column chooser, text size, row height, fit columns, wrap names, notes pages. 22 engine tests; UI tests for nested menus, hide/unhide, printed columns, ribbon fit at 1280/1600 px, Setup page.

### 2026-10-24: Session 6, continued (0.3.6 feedback)
- User feedback on 0.3.5 (4 screenshots): wrong fill icon and no font family; empty space on the Task tab; "Check again" gives no feedback; wants a GitHub link; wants MS Project's timeline zoom box; the Network Diagram lacks a legend and print layout; do all of 0.3.6 (every view printable, the rest of the 0.3.4 limits).
- Plan and questions in [PLAN-0.3.6.md](PLAN-0.3.6.md).

### 2026-10-24: Session 6, continued (0.3.6)
- Answers: fonts as listed; Task tab extras as proposed; network boxes as proposed; everything in 0.3.6; add GitHub, Releases and Report-an-issue links; explain terms (SS, FF…) for new users.
- Built: font family (`n:` style), bucket icon, Task tab Schedule/Editing groups, ribbon folds per group from the right, `open_url`, Check-again spinner + time, Getting Started + Glossary dialogs, link-type labels in details, timeline zoom box (`inView`, `gantt-range`), Network (`NetBox`, `NetDraw`, `NetLegend`, Compact/Standard/Detailed, driving links, labels, zoom/fit/pan, mini-map, path highlight; engine `NetLink.driving/lag`), printing for every view, xlsx styles (`styled`, `task_style`), `cells.ts` series + Paste Special in both grids, Sheets fill handle, Ctrl+D/R, freeze panes. 23 engine tests; UI tests for every view's print pages, network legend, Help links, glossary, fonts.
- Not done: the task sheet and Sheets are still two components (they now share `cells.ts` and `fmt.ts`); full icon redraw; network layout by WBS. Moved to 0.3.7.

### 2026-10-24: Session 6, continued (0.3.7)
- User: Gantt and Tracking looked identical; wanted Cairn's whiteboard network look and cards; man-hours with 8-hour shifts and resources in the templates; finish the 0.3.7 carry-overs.
- Built: Tracking Gantt (view switches table, baseline grey bars, solid done part, % labels, banner + Set Baseline), templates with people/assignments/costs and Man-hours title, `network::layout_with` (summaries, WBS bands) + whiteboard cards, icon redraw, `selection.ts` shared by both grids (Sheets: multi-range, row/column selection, aggregates). 25 engine tests; UI tests for Tracking, WBS network, whiteboard.

### 2026-10-24: Session 6, continued (0.3.8)
- User: printed Gantt had no link lines; wanted a date picker for dates; asked to record the workflow.
- Built: link paths in `print.tsx` `Strip` (same elbow rules as the canvas), `DateBtn` (ui.tsx, uses `showPicker()` on a hidden `<input type=date>`) on the active date cell in the grid (`.gdate`) and on date fields in the details pane (`.dfield`). UI tests for both. Added "How we work" to this file.

### 2026-10-24: Session 6, end (family)
- User is starting Datum Plot (GraphPad Prism + OriginPro) in a new repo and chat, with the same style and workflow. Wrote `docs/FAMILY.md` (stack, colours with accents per app, type, logo rules, layout, components, icons, charts, printing, updates and releases, docs, workflow, new-app checklist) and a draft `docs/brand/datum-plot-icon.svg` (teal).

### 2026-10-24: Session 6, end (README)
- README rewritten as an independent product (no comparisons with other tools), with new screenshots (Gantt, Tracking, Network, Dashboard) taken from 0.3.8; old images removed. "Pending and next" written below, including 4 bugs found while taking the screenshots.

### 2026-10-24: Session 6, end (Datum Atlas brief)
- User created Datum-Plot and asked for a third app: materials selection and property charts (the scope of Ansys Granta Selector, but easier and better). Wrote `docs/briefs/DATUM-ATLAS.md` (concepts, features, window, architecture, data model, release plan, quality bar, questions), a draft logo `docs/brand/datum-atlas-icon.svg` (violet), and added Atlas to FAMILY.md. Recommended: name Datum Atlas, original referenced data (never copy proprietary databases), no Python inside the app.

### 2026-10-25: Session 7 (0.3.9, ? help)
- User: every Datum app gets a small ? beside its tools that says what the tool is for and how to use it (built first in Datum Atlas 0.1.1); no separate teaching mode.
- Built: `ui/src/help.tsx` copied from Atlas with 46 Plan topics; `G` (ribbon group) takes `h`, `Page`/`Tools` take `help`, `Dialog` takes `help`; ? in the details pane and Project Setup; Help › What Is This Tool?; `i-help` icon; the old ad-hoc `button.help` style replaced by the family `.help`/`.helpcard`. Found by the new UI test: a dialog with no input and no OK kept focus outside, so Escape did nothing; it now focuses its Close button. UI tests: ? beside Leveling opens its card, every Task-tab group has a ?, What Is This Tool? opens a card, ? beside a page title.
- The 4 bugs listed in §5 are still open (moved to 0.3.10).

### 2026-10-25: Session 8 (0.3.10, Roadmap)
- User: Plot and Atlas have a Roadmap tile on Home; add the same to Plan. Another session had already released 0.3.9 (? help), so this shipped as 0.3.10. Built `roadmap.tsx` (Home tile + File sidebar item), fixed the print fit-to-pages leftover page (`W = TW / (fit − 0.35)`) and lightened the network cross. UI tests for both. The roadmap lives in `docs/site.json` (imported by `roadmap.tsx`, `resolveJsonModule` on); the DATUM hub site will read the same file. Wrote the hub brief `docs/briefs/DATUM-HUB.md` and added DATUM to FAMILY.md.

---

### 2026-10-27: Session 8, end (open-source comparison)
- User asked for a quick comparison with ProjectCraft (github.com/UCS-Care-Group/projectcraft, Rust + egui, 0.0.1 pre-alpha, MS Project parity). Verdict: Datum Plan leads on features and polish; ProjectCraft leads on rigour (about 550 tests, date parity fixtures, CI gates, a command line). Wrote the "Hardening plan" in §5; nothing built yet.

### 2026-10-27: Session 9 (0.3.11, hardening quick wins)
- User: build 0.3.11 with the two bugs and the hardening quick wins; record everything here, in the CHANGELOG and in the DATUM hub memory.
- Built `crates/core/src/hardening.rs` (4 tests). On first run it found real faults: a damaged XER calendar sliced past the end (`pnode`, now clamped and depth-limited) and a WBS that loops back on itself recursed for ever (stack overflow; `walk` now visits each WBS node once). A random-plan "late before early" report turned out to be 17:00 vs 08:00 next morning (same working moment); the test compares in working time.
- Fixed the Dashboard 0 % (`Calc::progress_date`, `Summary.measured_at`, label and report header say the date used) and the date picker (input blurs after the pick; UI test). `St::lock()` recovers a poisoned mutex. `deny.toml` + `license = "MIT"` in both crates; CI jobs `deny` and `platforms` (Windows, macOS). SECURITY.md, CONTRIBUTING.md. Decided not to enforce rustfmt (compact style).
- Roadmap renumbered: 0.3.12 Trust in the dates, 0.4.0 Risk & resources, 0.4.1 Progress history, 0.4.2 Command line & automation (also in `docs/site.json`). Stale "templates have no resources" limit removed.

### 2026-10-28: Session 10 (site.json about and steps)
- User (from the hub): add `about` and `steps` to `docs/site.json` and link the family page from the README. Checked each sentence against the code: MPX is export-only, so "open from XML, XER, MPX" became "open from XML and XER or from Excel and CSV; export to Excel with formulas, XML, XER and MPX". README links https://edgar-mendonca.github.io/Datum/apps/plan.

### 2026-10-28: Session 11 (0.3.12, trust in the dates)
- User: build 0.3.12. Wrote `cases.rs` (hand-worked cases, dates worked out before running), `parity.rs` + `tests/parity` (hand-made XML and XER with their own dates; a deliberate wrong date is reported), and four new hardening tests (links hold + tight forward pass on decorated random plans, row-order independence, random edit sequences, damaged CSV/xlsx).
- Found and fixed: (1) a started successor's late start used its whole duration, so its predecessor showed negative float under retained logic; finished successors also limited predecessors; (2) ALAP tasks took their total float and could break a successor link (and a mixed-calendar negative lag case), now the largest shift within free float that every link allows. My own arithmetic was wrong in five expected values (checked each against the rules before changing the test).
- Ran the random tests 10× longer in release: clean. README gained "Dates you can trust"; site.json roadmap: 0.4.0 is next (with the import report).

### 2026-10-28: Session 12 (0.4.0, risk and resources)
- User: build 0.4.0. Built `risk.rs` (+ `dmin`/`dmax` task fields, `Risk.days`), usage by group with capacity and `person`, `level::preview`, import notes in both readers + `compare_dates`, XER activity types; window: Schedule Risk view and sidebar item, Delay (days) column, Resource Graph People/Trades and Hours/People, levelling preview dialog, import report dialog, help cards. 6 engine tests and 8 UI checks added.
- Checked in the real app (Engineering template): Schedule Risk (1 % chance of the planned 15 Feb; P50 19 Feb, P80 23 Feb; parts lead time drives the finish), levelling preview (5 tasks move, finish 15 → 19 Feb, overallocated days 4 → 0), trades in people. Found and fixed there: stale bars and labels in Resource Graph (`<For>` over numbers → `<Index>`), and the drivers chart skipping every other task name (`autoSkip: false` on horizontal bars).
- The first release run failed on Windows only: git checked the parity files out with CRLF, so the import-report test's own edit did not match (the readers trim `\r` and passed). The test now normalises line endings and `.gitattributes` keeps the parity files LF; checked locally with CRLF copies.
- Not built from the old 0.4 list: manpower planned vs actual (needs actuals history, moved to 0.4.1) and the shared resource pool (moved to 0.6 with the portfolio). Schedule Risk is not on the Print page yet.

### 2026-10-28: Session 13 (0.4.1, progress history)
- User: build 0.4.1. Built `progress.rs` (manpower, earned schedule, look-ahead, credit library), `report::history` (actual through snapshots), task quantities, `CreditRule`/`LogItem` with `Op::Credit`/`ApplyCredit`/`Log`; window: Manpower and Look-ahead views (sidebar + Progress tab), Rules of Credit dialog, earned-schedule KPIs on S-Curves and Dashboard. 5 engine tests (59 in all) and 5 UI checks.
- Checked in the real app (Engineering template): look-ahead from the status date with ready flags, adding a constraint (found and fixed the task box showing the first task: `<option selected>`; same fix in Resource Graph and the progress report's period list), Rules of Credit → Concrete steps in the Steps tab, Manpower (forecast line made straight: the smoothed curve overshot). My look-ahead test first expected an unstarted task before the status date; with a status date it moves after it (engine right).
- Not done: Manpower "Done" is earned hours, not timesheet hours; the look-ahead and Manpower are not on the Print page yet.

## 5. Known limits and next steps

### Hardening plan (from the ProjectCraft comparison, 2026-10-27; not started)

Goal: match their rigour while keeping our lead on features. Quick wins done in 0.3.11; items 6–9 in 0.3.12; 10–12 are 0.4.2.

**Quick wins (hours each)**
1. ✅ (0.3.11; rustfmt left out, see §2) `cargo fmt --check` and `cargo deny` (licences, advisories) in CI; add `deny.toml`.
2. ✅ (0.3.11) CI on Windows and macOS too (Tauri builds there already); release stays Windows first.
3. ✅ (0.3.11; remaining `unwrap`s are guarded invariants or fixed dates) Never-crash pass: grep `unwrap()`/`expect()`/`panic!` outside tests in `crates/core` and `src-tauri`, replace with errors the window shows.
4. ✅ (0.3.11) A failed op changes nothing: one test that every op on bad input leaves the project equal to before.
5. ✅ (0.3.11; 2,000 tasks only) Speed gate: a test that 2,000 tasks reschedule in < 20 ms and 10,000 open in < 1 s (release build, CI).

**Trust in the dates (days each)**
6. ✅ (0.3.12: 47 tests, 47 hand-worked cases; keep adding a case per rule touched) More engine tests: aim for 300+. One file per area (calendars, constraints, link types, lags in %, levelling, progress override, retained logic, from-finish, elapsed). 
7. ✅ (0.3.12, own generator instead of `proptest`) Property tests (`proptest`): random plans never panic, early start ≤ late start, float ≥ 0 off the critical path, schedule is the same twice.
8. ✅ (0.3.12, hand-made files; real exports still wanted) Parity fixtures: sample MSP XML and XER files with the dates MS Project / P6 give them; a test checks we give the same dates. Show "Checked against N sample plans" in the README.
9. ✅ (0.3.12; the import report moves to 0.4.0) Hostile-file tests for XML, XER, MPX, xlsx import: damaged, huge, odd values; the import report lists what was changed or dropped.

**Reach (a week each)**
10. `datum-cli` (new `apps/cli` or `src-cli`): open, schedule, run ops, export, print the schedule as a table. Uses `crates/core` only.
11. Every action is an op with a name and a JSON schema (`ops list`); the CLI, the ribbon and Tell Me share them. Then an MCP server so AI agents can build and update plans.
12. Exact money: store costs in minor units (integers), not floats.
13. ✅ (0.3.11, no separate threat model yet) Docs they have that we lack: SECURITY.md, CONTRIBUTING.md, a threat model (files hold people's names; the only network call is the updater).

**Keep (where we lead):** the window (ribbon, sidebar, 18 views), tracking and EVM, reports and print, formulas and Sheets, XER and P6 power features, in-app updates, the Datum family.

### Pending and next (written at the end of 0.3.8)

**Bugs found while taking the 0.3.8 README screenshots (fix first, 0.3.10):**
1. ✅ (0.3.10) Print, "Fit timescale to 1 page wide": when the table takes most of the page (the templates now have Man-hours and Cost columns), a second near-empty page appears. Cause: the shrink in `print.tsx` `layout()` aims at 35 % of the *old* width for the chart, but the chart minimum is 30 % of the *new* width. Use `W = TW / (fit − 0.35)` when `TW > (fit − 0.35) × W`.
2. ✅ (0.3.10) The Network Diagram's completed-task cross is too heavy and runs over the text: lighten it (opacity) or use a corner tick.
3. ✅ (0.3.11) The Dashboard shows "Actual progress 0 %" when no status date is set even though tasks are done: fall back to today (or the latest actual date) and say so.
4. ✅ (0.3.11, blur after the pick; still to confirm on Windows) On Linux the date picker stays open after a pick: check on Windows and close it after choosing.

**Not yet checked on a real Windows PC or against real files:**
- Print to PDF in WebView2 for every view (flowing report pages, page numbers from `@page` margin boxes).
- XER, PMXML and MPX against files from other tools; custom fields and codes are not yet written to XER/XML.
- The clipboard with real Excel; the Aptos font.

**Features still to build (roadmap):**
- ✅ 0.4.0 Risk and resources (schedule risk, trades, levelling preview, import report). Left for later: manpower planned vs actual (0.4.1), a shared resource pool across plans (0.6).
- Progress history: the actual S-curve from snapshots (not a straight line); earned schedule; rules of credit and installed quantities; a 3-week look-ahead with a constraint log.
- Schedule quality: a health score with trend; baseline compare with ghost bars; a logic change log; a milestone trend chart.
- Beyond other tools: what-if scenarios (branch, compare, merge); delay analysis (as-planned vs as-built, time impact, windows); line of balance and takt for repetitive work; an optional assistant (review the schedule, draft a plan, write the report narrative).
- Portfolio: master plans and subprojects, cross-project links, a portfolio dashboard.
- Hardening: 100k-task speed (row windows), code signing, signed updates, crash reports.
- Working together: shared-folder check-out per WBS, comments, audit trail, field updates from a phone.
- Finishing: a text-styles editor (by task type), saved custom views, a user guide for 1.0.

**Known limits:**
- Manpower "Done" is earned hours from progress, not hours booked (no timesheets). Look-ahead, Manpower and Schedule Risk are not on the Print page yet.
- Actual progress is modelled linearly between stored periods (snapshots); without snapshots, linearly from the actual start to today.
- Schedule Risk results are not on the Print page yet, and are not saved with the plan (run again after opening).
- Windows SmartScreen warns on the first install (unsigned). Updates after that are silent.
- XER/PMXML/MPX exports are checked against our own readers only; needs real P6 and MS Project tests. Custom fields and codes are not yet written to XER/MSPDI.
- The grid fetches all visible rows per change (about 8 MB JSON at 20k tasks); fine to ~20k, 100k needs row windows (0.6.1).
- Not built yet: the reference's C2 scenario tests. The task sheet and Sheets share one engine but keep two renderers. Report pages print as flowing HTML (no per-page preview).
- Fill handle and paste were checked by code review and in the harness, not with a real clipboard in the app.
