# Changelog

## 0.3.5 — 2026-10-24

A cleaner ribbon, Setup in the sidebar, full control of columns, and printing that includes every column.

**Ribbon**
- The Task tab is split into three: **Task** (clipboard, font, alignment, insert, properties), **Schedule** (links, outline, manual/auto, split, leveling, critical and longest path, find, global change), and **Progress** (% complete, Mark on Track, Update Progress, status date, reporting periods, baselines, snapshots, progress reports).
- No more sideways scrollbar. On a narrow window the ribbon tightens, then shows icons only, so every command stays in view and group names are no longer cut off.
- New icons for Setup and Notes.

**Project Setup**
- Setup is now the first icon in the left sidebar and opens as a page, not a panel over the plan. New plans open on it. Skip or Done takes you to your chosen view.

**Columns**
- Fixed: the Insert Column categories (General, Schedule, Progress…) now open their list of fields. Menus can have any number of levels. "Search all fields…" is at the top.
- Right-click a column heading:
  - Hide Column (the column stays in the table, hidden);
  - Unhide Columns ▸ (lists the hidden ones, or Unhide All);
  - Delete Column (with undo). For a custom or formula column it asks whether to remove it from the table or delete the field and all its values.
- New Columns… dialog (heading menu, View › Data, Format › Columns): all fields on the left; the table's columns on the right with Show, width, order and remove.

**Printing**
- Every column of the table prints by default (Task Name, Duration, Start, Finish, Predecessors, Resources, % Complete…), not just the first three.
- Fixed: the chart no longer overlaps the last table columns.
- A table wider than the page continues on the next page without cutting a column in half. ID and Task Name repeat on every page.
- "Fit timescale to N pages wide" now really fits: it shrinks the page when the table is wide.
- Page Setup › View:
  - choose and order the columns to print;
  - text size and row height;
  - shrink columns to fit;
  - wrap long task names;
  - print task notes on the last pages.

 2026-10-24

Printing and PDF done properly, Excel-style text formatting, and a clearer Options page.

**Print and PDF (File › Print, Ctrl+P, or Report › Page Setup)**
- A new Print page like MS Project's: settings on the left, a live preview of every page on the right.
- Print the entire project or specific dates (from and to), and all pages or only some (e.g. 1-3, 5).
- Paper A4, A3, A2, A1, Letter, Legal or Tabloid, landscape or portrait.
- Scaling: adjust to a percentage, or fit the timescale to a number of pages wide.
- Margins in millimetres.
- Header and footer with left, centre and right parts. Codes for page number, total pages, date, project, company, client, manager, status date, project start and finish. The company logo and a client logo can go on either side.
- Legend on every page, the last page or none, with your own text.
- Columns: all of them or the first few, repeated on every page or only on the first. Gridlines, ID column and timescale units are adjustable.
- Pages are drawn as vector graphics, so the Gantt chart and text stay sharp in PDFs. Choose "Save as PDF" in the print window.
- The page setup is saved with the plan.

**Text formatting (Task tab › Font and Alignment)**
- Bold, italic, underline, strikethrough, font size, font colour, fill colour, and left, centre or right alignment for selected cells. Select whole rows to format the row.
- Ctrl+B, Ctrl+I and Ctrl+U, Clear Formatting, and Format Painter (Clipboard group).
- Works the same in the task sheet and in Sheets tabs. The formatting is saved with the plan and printed.

**Options**
- Fixed the Options page, where the labels and boxes had swapped places.
- Grouped into sections, each with an explanation.
- New options: reopen the last plan at start; the view plans open in; manual or auto scheduling for new tasks; the week-end and month-close day for reports; default paper and orientation; choose the workspace folder directly.

 2026-10-24

**Fix (critical):** in 0.3.2, after the first click the task sheet kept the same row selected. Clicking another cell only changed the column, so you could not select, edit or work on other tasks. Clicking now selects the cell you click. An automated test clicks cell after cell and edits one, so this can't come back unnoticed.

## 0.3.2 — 2026-10-24

Progress updating, a report builder and power tools, plus the fixes from your 0.3.1 screenshots.

**Fixes**
- The big Gantt Chart and Task buttons are now MS Project split buttons: click the top half to run, the arrow below for the menu.
- Icons on every right-click menu item (column headings and tasks).
- The Format tab sits under its own "Gantt Chart Tools" label, no longer squeezed together.
- The Gantt Chart Style gallery is back on the Format tab (10 styles), with Bar Styles and Gridlines.
- The File page sidebar is blue again (it had turned black).
- The Sheets view no longer fails on plans without sheets.

**New**
- Update Progress (Report › Update Progress or the sidebar): every task due in the period with planned vs actual; enter actual start and finish, % complete, remaining and a note; "Set all to planned %"; Close Period stores a snapshot.
- Report Builder: build your own report from sections (key figures, S-curve, period bars, tables with chosen columns and filters, notes, behind-schedule list). Templates: Weekly management, Monthly client, Schedule health, Cost report. Print to PDF or save as HTML.
- Relationship Diagram (View tab): the selected task with its predecessors and successors; driving links in bold; click a box to follow the chain.
- Recurring tasks (Task › Recurring Task): every n days, weeks or months, for a number of occurrences.
- Global Change (Task tab or a heading's menu): set one field to a value for every selected task or every task in the view, as one undo step.
- Conditional formatting per column: colour cells by rules (font colour, fill and bold), e.g. negative float in red.
- Portfolio shows plans in subfolders as an EPS tree.
- Excel export keeps formulas: formula columns stay live Excel formulas, dates and durations are typed, sheet tabs and a Plan sheet are included.
- Automated UI tests run on every build.

## 0.3.1 — 2026-10-24

A rebuilt foundation: a real spreadsheet grid, Excel formulas, period reporting and a cleaner look.

**A spreadsheet that works like Excel**
- The task sheet is rebuilt. Header and rows share one scroll area, so columns always line up (fixes the misaligned headings).
- Select cells like in Excel: click and drag, Shift+click, Ctrl+click for several ranges, click a column heading for the whole column or the ID column for whole rows, Ctrl+A for everything.
- Delete clears the selected cells (Delete on whole rows, or Ctrl+−, deletes tasks). Type and press Ctrl+Enter to fill every selected cell. Ctrl+D fills down, and the fill handle at the corner of a selection drags values down.
- Copy and paste blocks of cells to and from Excel. Pasting below the last task adds new tasks.
- Formula bar above the sheet shows the active cell, and Sum, Average and Count of the selected numbers.
- Double-click any column heading to rename it. Right-click a task and choose Rename Task, or press F2.
- Excel-style filter buttons on every heading: sort, tick the values to show, custom filter.
- The task name and ID columns stay in view when you scroll sideways.

**Formula columns (Excel formulas, by column, as in MS Project)**
- New Formula Column (right-click a heading, the fx button or Format › Formula Column): name it, choose Number, Text, Date, Duration, Cost or Yes/No, and write an Excel formula that refers to other columns by name, e.g. `=[% Complete]-[Baseline % Complete]` or `=IF([Finish]>[Baseline Finish],"Late","On time")`. It is calculated for every task.
- Powered by the GridCraft engine: about 400 Excel functions (IF, IFS, SUMIFS, XLOOKUP, WORKDAY, NETWORKDAYS, TEXT, ROUND, LET…). Formula columns can use each other.
- New fields: Baseline % Complete (where the baseline says a task should be at the status date) and % Complete Variance.

**Sheets: spreadsheets inside your plan**
- Add sheet tabs for quantities, estimates or progress measurement, with Excel formulas, number formats, copy and paste.
- Sheets read your tasks through the Plan sheet, e.g. `=SUMIF(Plan!I:I,100,Plan!M:M)`.

**Progress reporting by period**
- Reporting periods are yours to set per plan: the weekday a week ends on, the day a month closes, and how progress is weighted (duration, cost, hours or physical %). Set them in Project Setup or Project › Reporting Periods.
- Weekly and monthly Progress report for any period: actual and planned progress with the change since last period, progress this period vs planned, variance, forecast finish, SPI and CPI; charts; progress by phase last period vs this period; completed, started, in progress, behind baseline, look-ahead and milestones.
- Notes for each period: executive summary, progress, issues, actions, look-ahead and photos, saved inside the plan and printed with the report.

**S-curves and charts**
- Interactive charts in a clean, GraphPad Prism-like style: hover anywhere to read every value, a status-date line, colour-blind-safe colours, and a PNG button on every chart.
- S-curves page: project progress, progress per period, cost and hours S-curves, one S-curve per phase, and the data table; weekly or monthly, weighted as you choose.
- The Dashboard now shows KPIs with the change since last period, the project S-curve, progress per period, progress by phase, cost and earned value, and a manpower histogram.
- Reports saved as HTML keep their charts as pictures.

**Look and feel**
- The CentriForge design language on the MS Project layout: IBM Plex fonts, a calmer palette and square, clean controls.
- Light, Dark and Auto themes (Auto follows Windows). Switch from the title bar or File › Options.

**Fixes**
- Right-click menu commands did nothing in 0.3.0; they work now.
- Selecting a cell sometimes highlighted the wrong column.

## 0.3.0 — 2026-10-24

The biggest release so far: the MS Project look and feel with Primavera P6 power, rebuilt from the engine up. Everything planned for 0.2.1 to 0.5.2 is in this one release, including the network diagram.

**A new window (SolidJS)**
- MS Project ribbon: Task, Resource, Report, Project, View and the Gantt Chart Tools Format tab, with dropdown and split buttons (Gantt Chart, Filter, Group by, Sort, Tables, Outline, Timescale, Baseline, Export).
- Tell Me (Ctrl+K or Alt+Q) searches every command. Quick access toolbar with Save, Undo, Redo and Print.
- Task sheet at MS Project density (20 px rows): drag columns to reorder, resize, double-click a heading for Field Settings, right-click a heading to insert, hide, sort, group or filter, and an "Add New Column" picker over about 450 fields.
- Inline editing with choice lists, Tab/Enter navigation, type-to-edit, a blank row for new tasks, indicators for late and manual tasks, and a task mode column.
- Status bar with filter state, task count, project figures, view shortcuts and a zoom slider.

**Fields, custom fields and formulas**
- About 450 task fields: general, schedule, progress, work and cost, 11 baselines (start, finish, duration, cost, work), variances, earned value, and custom Text1–30, Number1–20, Flag1–20, Date, Cost, Duration, Start, Finish and Outline Code fields.
- Custom Fields dialog: rename (alias), lookup tables with restricted values and defaults, MS Project formulas (`IIf`, `Switch`, `ProjDateDiff`, `DateAdd`, 34 functions, `[Field]` references), summary rollups and graphical indicators.
- Activity codes (P6) with values and colours: show as columns, group and filter by them.
- Inspect shows every field of a task.

**Views**
- Gantt Chart and Tracking Gantt with critical path, slack, late bars, baseline 0–10, splits, deadlines, today and status lines, bar labels, and drag to move, resize, link or set progress.
- MS Project timeline across the top.
- Network Diagram (MS Project style boxes, critical links, progress crosses, milestones).
- Calendar, Task Usage, Resource Usage (days, weeks, months, quarters; work or cost; overallocation in red), Resource Sheet, Resource Graph and Team Planner.
- Tables: Entry, Schedule, Tracking, Cost, Work, Variance, Earned Value, Summary, Baseline and P6 Activities, plus your own.
- Filters (20 built-in plus your own, with and/or conditions, related summaries and highlight mode), groups (several levels, with rolled-up dates and costs) and sorts that keep the outline. Outline levels, expand and collapse, project summary task.

**Primavera P6 power**
- Details pane with P6 tabs: General, Status, Resources, Predecessors, Successors (with driving links and float), Codes, Steps (weighted physical %), Notebook, Expenses, WPs & Docs, Custom Fields, Risks and Notes.
- Activity types (task dependent, resource dependent, level of effort, start/finish milestone, WBS summary) and % complete types (duration, units, physical).
- Schedule options: retained logic or progress override, critical by float or longest path, lag calendar, float paths, schedule from finish, open ends.
- Elapsed durations (`3ed`), percentage lags (`2SS+50%`), task splits, priorities and leveling delay.
- Resource leveling (standard, priority or ID order, within slack) and clearing it.
- Task types: fixed duration, fixed units and fixed work, effort driven.

**Costs, progress and earned value**
- Cost rate tables A–E with dated rate changes, overtime rates, availability periods and resource calendars.
- Fixed costs and expenses with start, prorated or end accrual.
- 11 baselines, Update Project, Store Period Performance (snapshots) and trend reports.
- Earned value per task and project: PV, EV, AC, SV, CV, SPI, CPI, BAC, EAC, VAC, TCPI.
- Currencies: any ISO 4217 code, chosen per plan, with symbol and decimals.

**Reports and dashboards**
- Dashboard: complete, finish, cost, SPI, CPI, late and critical counts, cost S-curve (planned, forecast, earned, actual), progress and cost by phase, milestones and late tasks.
- Report gallery: Project Overview, Weekly/Monthly Progress, Earned Value, Cost Overview with cash flow, Work Overview, Critical Tasks, Late & Slipping, Milestones, Upcoming Tasks and Trend.
- Your logo and company name on every report. Print or save as PDF, save as HTML, save the Gantt chart as a PNG picture.
- Check Schedule: the DCMA 14-point assessment plus P6 checks (long lags, SF links, soft constraints, manual tasks, summary links, out of sequence, missed deadlines), with your thresholds; click a check to filter its tasks.
- Risk register with probability × impact scoring, a heat map and links to tasks.
- Compare Projects: added, removed and changed tasks against another .dplan, XML or XER file.
- Portfolio: every plan in a folder with its dates, progress, cost, SPI, CPI and late tasks.

**New projects**
- Project Setup panel on the right (skippable): project details, Activity ID numbering (A1000, step 10), calendar, currency, WBS typed as an indented list, resources, and branding.
- Change Working Time, Project Information (with statistics), Resource Information (costs, availability, notes) and Task Information dialogs.

**Files**
- New export formats: Primavera P6 XML (.pmxml), MPX, iCalendar (.ics), HTML and JSON. Excel and CSV export the current table's columns.
- JSON import. Plans from 0.2 open as before; a copy is kept as `name.backup.dplan` before they are upgraded.

**Updates**
- Opt in to beta versions in Options.

**Under the hood**
- The engine computes every field, filter, group and sort, so the window stays fast with large plans (20,000 tasks schedule and display in about 0.15 seconds).
- 18 engine tests, including earned value, formulas, leveling, splits, schedule from finish and the schedule checks.

**Coming in 0.3.1:** report designer, Page Setup, bar and text style editors, recurring tasks, Relationship diagram, global change, an EPS tree, and custom fields and codes in XER and MS Project XML.

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
