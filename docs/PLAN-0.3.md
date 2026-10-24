# Datum Plan 0.3: brainstorm and plan

> Status: **proposal for review**. Nothing here is built yet. Mark it up, then we build it.
> 0.3 merges the old 0.2.1 → 0.5.2 roadmap steps (network diagram included) into one release, as asked on 2026-10-24.

---

## 1. Honest audit of 0.2.0

| Area | What is wrong today |
| --- | --- |
| **Look** | It does not feel like MS Project or P6. Rows are too airy, there are no column dropdown arrows, no indicator or Task Mode columns, and no Format tab. The ribbon is a flat list of buttons. |
| **Ribbon** | Some buttons do nothing useful. "Gantt Chart" should be a dropdown of views, as in MSP. There are no split buttons (Paste ▾, Mode ▾, Task ▾), no Font group and no contextual tabs. |
| **Getting information** | No MSP-style Task Information dialog, only the bottom pane. No Resource Information or Project Information with statistics. No "Inspect" to explain why a task starts when it does. |
| **Fields** | About 30 fields. MSP has about 400 task fields plus about 200 custom fields (Text1–30, Number1–20, Flag1–20 and more). There is no Field Settings, no Custom Fields dialog, no formulas, lookups, rollups or graphical indicators. |
| **Column header menu** | Missing MSP's Cut / Copy / Paste / Paste Special / Wrap Text / Insert Column / Hide Column / Font / Text Styles / Field Settings / Custom Fields / Data Type. |
| **Organising** | No filters, groups, sorting, highlight filters, Show Outline Level, saved tables or saved views. |
| **Formatting** | No fonts, cell or row colours, bar styles, text styles, gridlines, timescale dialog or link styles. |
| **Views** | Missing Tracking/Detail Gantt, Task Usage, Resource Usage, Resource Graph, Network Diagram, Team Planner, Task/Resource forms, and split views. |
| **Reporting** | Two basic pages. No report designer, report gallery, charts, report export or proper period reporting. |
| **Updating progress** | No Update Tasks / Update Project dialogs, multiple baselines, earned value, progress lines or period snapshots. |
| **Timeline** | Basic. MSP's timeline has Start/Finish labels, a draggable window, several bars, callouts and its own Format tab. |
| **Files** | No Save As type list. Missing P6 XML, MPX, PDF, PNG, ICS and HTML. |
| **Navigation** | Too much jumping between places: details are in one spot, settings in another, and views and tabs are not linked. |

---

## 2. Design principles for 0.3

1. **MS Project look first, P6 power underneath.** Same ribbon tabs, same dialogs and names, and the same right-click menus. MSP users should find everything where they expect it.
2. **One click to information.**
   - Double-click a task → Task Information (MSP tabs).
   - Double-click a resource → Resource Information.
   - Double-click a column header → Field Settings.
   - Double-click a bar → Format Bar.
   - Double-click a link → Task Dependency.
3. **Everything is a view.** View = table (columns) + filter + group + sort + screen (Gantt, Usage, Network…). All views are saved in the plan, with built-ins as in MSP's Global template.
4. **Every ribbon button works.** Dropdowns and split buttons where MSP has them, and a contextual Format tab for the active view.
5. **Engine in Rust.** Fields, formulas, filters, timephased data, leveling, earned value and network layout are all core code with tests. The UI only draws.
6. **Fast with 10,000+ tasks.** The core sends only the visible columns and rows.

---

## 3. Feature catalogue for 0.3

### A. Shell, ribbon and navigation (MS Project layout)
- **Tabs:** File · Task · Resource · Report · Project · View · Help, plus contextual **Format** tabs (Gantt Chart Format, Network Diagram Format, Timeline Format, Usage Format).
- **Task tab**
  - View ▾: the MSP view list
  - Clipboard: Paste ▾, Cut, Copy, Format Painter
  - Font: font, size, bold, italic, underline, fill, colour
  - Schedule: 0–100%, Mark on Track ▾, Respect Links, Inactivate, Indent, Outdent, Link, Unlink
  - Tasks: Manually / Auto Schedule, Inspect, Move ▾, Mode ▾
  - Insert: Task ▾, Summary, Milestone, Deliverable
  - Properties: Information, Notes, Details, Add to Timeline
  - Editing: Scroll to Task, Find ▾, Clear ▾, Fill ▾
- **Resource tab:** Team Planner ▾, Assign Resources, Resource Pool, Add Resources ▾, Information, Notes, Details, Level Selection/Resource/All, Leveling Options, Clear Leveling, Next Overallocation.
- **Report tab:** Dashboards ▾, Resources ▾, Costs ▾, In Progress ▾, Custom ▾, Recent ▾, Compare Projects, Visual/Export.
- **Project tab:** Subproject, Project Information, Custom Fields, Links Between Projects, WBS ▾, Change Working Time, Calculate Project (F9), Set Baseline ▾, Move Project, Status Date, Update Project, Schedule Options, Check Schedule.
- **View tab:**
  - Task Views: Gantt ▾, Task Usage, Network Diagram ▾, Calendar ▾, Other Views ▾
  - Resource Views: Team Planner ▾, Resource Sheet ▾, Resource Usage ▾, Other ▾
  - Data: Sort ▾, Outline ▾, Tables ▾, Highlight ▾, Filter ▾, Group By ▾
  - Zoom: Timescale ▾, Zoom ▾, Entire Project, Selected Tasks
  - Split View: Timeline ☐, Details ☐ ▾
  - Window: New Window, Switch, Arrange
- **Quick Access Toolbar** you can customise, "Tell me what you want to do" (our Ctrl+K), and a **status bar** with view shortcuts (Gantt, Task Usage, Team Planner, Resource Sheet, Reports), the zoom slider and the new-task mode.
- **Left view bar** (from Cairn) stays, with a vertical view name like MSP's "GANTT CHART" label, collapsible.

### B. Grid, fields and custom fields
- **Field catalogue, about 450 fields** grouped as in MSP:
  - Task: dates, durations, work, cost, progress, baselines 0–10, earned value, constraints, IDs and codes, flags, resources, notes, links…
  - Resource fields and assignment fields.
  - Each field has a type: text, number, duration, date, cost, flag, percentage or list.
- **Custom fields:** Text1–30, Number1–20, Flag1–20, Date1–10, Cost1–10, Duration1–10, Start1–10, Finish1–10, Outline Code1–10, for tasks and resources. The **Custom Fields dialog** (as in your screenshot) has:
  - Rename
  - Lookup table with default value and "restrict to list"
  - **Formula** with a field picker and functions:
    - arithmetic and comparison operators
    - `IIf`, `Switch`, `Choose`, `ProjDateDiff`, `ProjDateAdd`, `DateDiff`, `Now`, `Abs`, `Round`, `Len`, `Left`, `Mid`, `InStr`, `IsNull`, `Format`
  - Rollup for summaries (Max, Min, Sum, Average, Count, or use the formula)
  - **Graphical indicators** (rules → icons/colours, separately for tasks, summaries and the project summary)
- **Column header menu** exactly as in MSP: Cut, Copy, Paste, Paste Special, Wrap Text, Insert Column, Hide Column, Font, Text Styles, **Field Settings** (title, alignment, width, header wrap), **Custom Fields**, Data Type.
- **Column dropdown arrows (AutoFilter):** sort A→Z / Z→A, group by this field, filter by values, date ranges and custom conditions.
- **Tables** (saved column sets): Entry, Cost, Schedule, Tracking, Variance, Work, Summary, Baseline, Earned Value, Constraint Dates, Delay, plus your own.
- **Indicator column** ("i") with notes, constraints, missed deadlines, overallocation, recurring and hyperlink icons, and tooltips. **Task Mode column** with pin/auto icons.
- **Cell and row formatting:** font, colour, fill, bold and so on. Text Styles by row type (critical, summary, milestone, late…). Format Painter.
- **Paste Special** (values only, or the whole task), Fill ▾ (down, up), Clear ▾ (formats, contents, notes, hyperlinks, all).
- Row height and wrap text, hyperlinks, and a notes editor with bold and lists.

### C. Information dialogs (one click to everything)
- **Task Information:** General · Predecessors (editable grid) · Resources (editable grid) · Advanced (constraint, deadline, task type, effort-driven, calendar, WBS code, earned value method, mark as milestone) · Notes · Custom Fields. Selecting several tasks edits them together.
- **Resource Information:** General (type, max units, availability over time, booking type), Costs (rate tables A–E with dates), Notes, Custom Fields.
- **Project Information:** dates, status date, calendar, priority and **Statistics** (current, baseline and actual start/finish/duration/work/cost, plus variance).
- **Task Dependency:** double-click a link line to change its type or lag, or delete it.
- **Inspect pane:** why a task starts when it does (driving predecessor, constraint, calendar, leveling delay), with a **Task Path** highlight of driving and driven tasks.
- The P6-style bottom details pane stays as an option (Details ☐).

### D. Scheduling engine
- **Task types:** Fixed Units, Fixed Work and Fixed Duration, with effort-driven scheduling (MSP rules: duration × units = work).
- **P6 types:**
  - Activity types: task-dependent, resource-dependent, level of effort, WBS summary, start milestone, finish milestone
  - Duration types
  - % complete types: physical, duration, units
  - Steps with weights
- Elapsed durations (`3ed`), % lags (`2FS+50%`), recurring tasks, split tasks (drag to split), and schedule from the finish date.
- **Resource calendars**, availability over time, and rate tables A–E over time. Overtime work and cost.
- **Resource leveling:**
  - Leveling Options dialog: automatic/manual, look-for-overallocation basis, order (ID / Standard / Priority), level only within available slack, allow splitting, priority 0–1000
  - Level Selection / Resource / All, Clear Leveling, Next Overallocation
  - Leveling Gantt showing the delay
- **P6 scheduling options:**
  - retained logic vs progress override
  - "critical = total float ≤ x" or the longest path
  - free/total float computed from finish or start
  - open-ended tasks warning
- **Longest path** and **multiple float paths** (P6): number of paths, and free-float or total-float based.
- **Move Project, Calculate (F9),** and automatic or manual calculation.

### E. Views
| View | What it shows |
| --- | --- |
| Gantt Chart / Tracking Gantt / Detail Gantt (slack bars) / Leveling Gantt / Multiple Baselines Gantt | Bar styles per type, progress lines, slippage |
| **Task Usage** and **Resource Usage** | Timephased grid per day/week/month: work, actual work, cost, baseline work, overallocation highlighted. Edit hours per period. |
| **Resource Graph** | Histogram of units or work against availability per resource |
| **Network Diagram** | PERT boxes (name, ID, start, finish, duration, resources, % complete; box styles configurable), critical path in red, layout options (top-down / left-right, by WBS), zoom, collapse summaries, drag to link |
| Relationship Diagram | The selected task with its predecessors and successors |
| **Team Planner** | Resources as rows, tasks as bars; drag work between people; an unassigned-tasks lane |
| Calendar | Month, week and day, with bars spanning days, as in MSP |
| Timeline | MSP style (see F) |
| Task Form / Resource Form / Task Details Form | The bottom split view as in MSP |
| P6 Activities layout | WBS bands, bottom detail tabs, activity codes |
| WBS Chart | Org-chart of the WBS |
| Sheets | Task Sheet, Resource Sheet |
| Split views | Any view on top of a form or usage view (Details ☐ ▾) |

### F. Timeline (as in your screenshot)
- "Add tasks with dates to the timeline" placeholder, with Start and Finish labels at the ends and Today marked.
- A **pan-and-zoom window** you drag to scroll and resize the Gantt.
- Tasks as **bars** or **callouts**, milestones as diamonds. Drag tasks from the grid onto the timeline.
- Several timeline bars, each with its own date range.
- Timeline Format tab: text styles, date format, show/hide dates, copy the timeline as an image.

### G. Organising: filters, groups, sort, highlight, outline
- **Built-in filters:**
  - All Tasks, Active, Completed, Incomplete, Critical, Late, Slipping, Milestones, Summary
  - Date Range…, Task Range…, Using Resource…, Should Start By…, Tasks With Deadlines
  - Overallocated, Unstarted, In Progress
- **Filter builder:** field / test (equals, contains, greater than, within, is between…) / value or a prompt "?", combined with AND/OR, optionally showing related summary rows.
- **Groups:** by any field, several levels, with intervals for dates and numbers, group header colours, and rollups.
- **Sort:** up to 3 keys, optional renumbering.
- **Highlight filters** instead of hiding rows. **Show Outline Level 1–9**, plus Show/Hide Summary Tasks and Project Summary Task (task 0).

### H. Progress, baselines and earned value
- **11 baselines** (Baseline, Baseline1–10) and **10 interim plans**. Set Baseline dialog: for entire project or selected tasks, roll up options.
- **Update Tasks** dialog (actual start/finish, actual and remaining duration, % complete) and **Update Project** (update work as complete through a date; reschedule incomplete work after a date).
- **Progress lines:** on the status date, at intervals, or at chosen dates.
- **Earned value:** BCWS/PV, BCWP/EV, ACWP/AC, SV, CV, EAC, BAC, VAC, CPI, SPI, TCPI, with physical % complete as the method. Earned-value tables and charts.
- **Period snapshots** (P6 "store period performance"): freeze the plan each week or month, with a comment and photos, for trend reports.
- Actual cost and work entry per period in the usage views.

### I. Costs
- Rate tables A–E with effective dates, overtime rates, and cost per use.
- Fixed cost with accrual (start, prorated, end). Expenses (P6 project expenses). Cost accounts. Budget resources.
- Cash flow per week, month or quarter, cumulative cost, and budget vs actual.

### J. High-end reporting
- **Report gallery** (Report tab), as in MSP plus extras:
  - Dashboards: Project Overview, Burndown, Cost Overview, Work Overview, Upcoming Tasks
  - In Progress: Critical Tasks, Late Tasks, Milestones, Slipping Tasks
  - Resources: Resource Overview, Overallocated Resources
  - Costs: Cash Flow, Cost Overruns, Earned Value Report, Resource and Task Cost Overview
  - **Periodic:** Weekly and Monthly Progress Report (with narrative comments), 3-Week Look-Ahead, Executive One-Pager, Baseline Variance, Schedule Health (DCMA), Period Trend (from snapshots)
- **Report designer** (MSP Report Tools):
  - A page canvas with tables (any fields, filter, group, sort), charts (bar, column, line, area, pie, S-curve, histogram, Gantt snapshot), text boxes with live fields (`[Project Finish]`), images, KPI tiles and page headers/footers
  - Drag to place, plus Format tabs for each element
  - Saved in the plan, and as templates you can share
- **Export** a report to PDF, PNG, Excel (data) and HTML. **Print** with page setup.
- **Visual reports:** pivot-style timephased exports to Excel.

### K. Analysis and quality
- **Check Schedule (DCMA 14-point):** missing logic, leads, lags, relationship types, hard constraints, high float, negative float, high duration, invalid dates, resources, missed tasks, critical path test, CPLI, BEI. Each check gives pass/fail with the list of tasks.
- **Schedule log:** open ends, out-of-sequence progress, constraints, loops, and tasks without resources.
- **Compare Projects:** two versions side by side, with changed dates, durations and links.
- **What-if scenarios:** copy the plan, change it, then compare finish and cost.
- **Risks and issues register:** probability, impact and score; owner; linked tasks; response; and a heat map.

### L. Multi-project (P6 enterprise)
- **EPS tree** and **project portfolio** on the Home screen. Each project stays a `.dplan` file, indexed by a workspace file (`.dpws`), so files stay portable.
- **Master project with inserted subprojects**, a shared **resource pool** across projects, and cross-project links.
- **OBS** (who is responsible), **activity codes** (project and global, with values and colours), and **UDFs** (via custom fields).

### M. Formatting and printing
- **Bar Styles** dialog (shape, pattern and colour for each task type and condition; text left, right, top, bottom and inside), Format Bar for one task, Text Styles, Gridlines, Layout (link style, date format, bar height), Timescale (3 tiers, units, labels, count, non-working time shading), and Format Painter.
- **Page Setup:** orientation, scaling (fit to N pages), margins, header, footer and legend with fields and logo, which columns to print, notes, and blank pages.
- **Print preview**, then print or save as **PDF**.
- **Copy Picture:** the current view to PNG or SVG.

### N. Files: open, save and export
| Format | Open | Save | Notes |
| --- | :-: | :-: | --- |
| Datum Plan `.dplan` | ✓ | ✓ | Native |
| MS Project XML `.xml` | ✓ | ✓ | Extended with custom fields, multiple baselines, rate tables and timephased data |
| Primavera P6 `.xer` | ✓ | ✓ | Extended with activity codes, UDFs, expenses and steps |
| **Primavera P6 XML** (PMXML) `.xml` | ✓ | ✓ | New; detected automatically next to MSPDI |
| **MPX** `.mpx` | ✓ | ✓ | New; older exchange format (MSP 98, others) |
| Excel `.xlsx` / CSV `.csv` | ✓ | ✓ | Choose a table or map columns |
| **PDF** | | ✓ | Print/report output |
| **PNG / SVG** | | ✓ | Copy Picture of a view |
| **iCalendar** `.ics` | | ✓ | Milestones and tasks for Outlook/Google Calendar |
| **HTML** | | ✓ | A self-contained report you can email |
| JSON | ✓ | ✓ | Open data for scripts and integrations |
| `.mpp` | ✗ | ✗ | Closed format (your decision). Save as XML from MS Project instead. |
- **Save As** shows a file-type list, as in MSP. **Open** accepts every format in the table.

### O. Small things that make it feel professional
- Rows 20 px high and a 9 pt font as in MSP, header dropdown arrows, a green selection border on the active column, and a row-number shade.
- Tooltips on every button with a description, as MSP does.
- Undo history list (Undo ▾), and a scheduling messages bar ("This task has a constraint conflict").
- Remember window, view, column widths and splits per plan.
- A "New Window" for the same plan, and side-by-side views.

---

## 4. Architecture changes

| Change | Why |
| --- | --- |
| **Field registry in Rust** (`fields.rs`): every field's ID, name, type, category, editable flag, getter and setter, for tasks, resources and assignments | One source for the grid, filters, groups, formulas, reports and export. Hundreds of fields without UI code for each. |
| **Formula engine** (`formula.rs`): an expression parser and evaluator using MSP syntax and `[Field]` references | Custom fields, filters and report text. |
| **Query API:** `rows(view)` returns only the visible columns for the visible rows, plus Gantt geometry | Speed with 10k tasks and 450 fields. |
| **View model in the plan:** tables, filters, groups, sorts, views, bar styles, text styles, page setup and reports, stored in `.dplan`. Built-ins live in a "global" template in app config. | As in MSP's Global.mpt. |
| **Timephased engine** (`timephase.rs`): per-period work, cost and actuals by contour; leveling and earned value use it | Usage views, graphs, cash flow, EVM. |
| `level.rs` (serial leveling), `evm.rs`, `check.rs` (DCMA/log), `paths.rs` (longest path, float paths), `network.rs` (layered layout) | The new analysis features, all with unit tests. |
| **Workspace** (`.dpws`) for EPS/portfolio, with links to `.dplan` files | Multi-project without a database server. |
| **PDF** through WebView2's native PrintToPdf (no extra library) | Small installer, accurate output. |
| **UI rebuild** (see decision 1 below): components for ribbon, dialogs, grid, canvas views and report designer | Today's string-template UI will not scale to MSP-level dialogs. |

---

## 5. How 0.3 gets built and checked

0.3 is one release, but it is too big to check all at once at the end. So:

- **Beta channel:** an Options switch "Get beta versions". When it is on, the updater also offers pre-releases (`0.3.0-beta.1`, `-beta.2`…). You can try each milestone without waiting, and 0.3.0 final goes to everyone.
- **Milestones** (each one is a beta):

| Beta | Contents |
| --- | --- |
| beta.1 | New UI foundation: MSP ribbon with dropdowns and the Format tab, status bar, dense grid with header dropdowns, indicator and Task Mode columns, field registry (about 450 fields), Field Settings, header menu, Tables, Task/Resource/Project Information dialogs, timeline as in MSP |
| beta.2 | Custom fields with formulas, lookups, rollups and graphical indicators; filters, groups, sort, highlight, outline levels; Views manager; cell/row formatting and text styles |
| beta.3 | Engine: task types, effort-driven, elapsed and % lags, recurring and split tasks, resource calendars and rate tables, leveling, P6 activity types and options, longest path and float paths, Inspect and Task Path |
| beta.4 | Task/Resource Usage, Resource Graph, Team Planner, Network Diagram, Relationship Diagram, better Calendar, split views, forms |
| beta.5 | 11 baselines and interim plans, Update Tasks and Update Project, progress lines, earned value, costs (fixed, expenses, cost accounts, cash flow), period snapshots |
| beta.6 | Reports: gallery, designer, charts, periodic reports, export to PDF/PNG/Excel/HTML; Bar Styles, Timescale, Page Setup, print preview, Copy Picture |
| beta.7 | Analysis: Check Schedule (DCMA), schedule log, compare projects, what-if, risks and issues; EPS workspace, master/subprojects, resource pool, activity codes; new file formats (P6 XML, MPX, ICS, HTML, JSON) |
| **0.3.0** | Fixes from your beta feedback, performance pass (10k tasks), documentation |

---

## 6. Decisions needed from you

1. **UI technology.** I recommend moving the screen layer from hand-written HTML strings to a small component library, **SolidJS** (about 7 KB), with a bundler. The engine and all logic stay in Rust. This is what lets dialogs, ribbons and the report designer reach MS Project quality without the code becoming fragile. The alternative is to stay as we are: possible, but slower and buggier at this size.
2. **Multi-project storage:** a workspace file plus `.dplan` files (recommended: portable, works with OneDrive and email), or a single database file like P6's.
3. **Beta channel:** OK to ship betas through the updater (opt-in) while 0.3 is being built?
4. **Priorities** if something must wait for 0.3.1: which of reports, network diagram, leveling, earned value or EPS matters most for your daily work?
5. **Screenshots that would help most:**
   - MSP: Task Information (all tabs), Field Settings, the Lookup/Formula/Graphical Indicators dialogs, Bar Styles, Timescale, Page Setup, the Report gallery and a report in the designer, the Network Diagram, the Team Planner, Resource Usage, the View ▾ dropdown, the status bar, and Timeline Format.
   - P6: Activities with the bottom tabs, Layout options, Activity Codes, Schedule Options, and Check Schedule.
