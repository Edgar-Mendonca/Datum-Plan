# AI Link: coverage of the window by tools

Every command in Datum Plan should have an AI Link tool, so the assistant never has to work on the screen. This list maps each ribbon group and view to its tools. ✅ has a tool · 🔜 planned (batch and version) · — no tool, on purpose.

Tools: 182 (0.9.13). The engine's tools are in `crates/core/src/assist.rs`; the window's own (its tabs, files and what it shows, `assist::WINDOW_TOOLS`; other plan files, `assist::MANY_PLANS`) are run by `src-tauri/src/ai.rs`.

## Batches

| Batch | Version | Theme | Status |
|---|---|---|---|
| 1 | 0.9.10 | Drive the window: views, tables, filter, group, sort, outline, select, zoom, Gantt options, Task Path, panes, new/open/save as/close plans, what the window shows | ✅ |
| 2 | 0.9.11 | Project setup and details: settings (start, calendar, hours per day, currency), schedule options, reporting periods, baseline lock, column titles, percent decimals, modules, bar colours, conditional formatting, report notes, task expenses/documents/notebook, resource rates/availability/leave, rules of credit, spelling, find and replace | ✅ |
| 3 | 0.9.12 | Sheets in full: insert and delete rows and columns, move sheets, merge, freeze panes, conditional formatting, cell notes, hide rows and columns, tab colour, sort a range, remove duplicates, pictures, row heights | ✅ |
| 4 | 0.9.13 | Many plans: links between plans, linked plans and what moved, update links, subplans, apply setup to subplans, scenarios, compare projects, portfolio, resource pool, import | ✅ |
| 5 | 0.9.14 | Print and reports: page setup, a view as PDF in Downloads, report builder, progress report notes | 🔜 |

## Home

| Group | Command | Tool |
|---|---|---|
| Clipboard | Cut, Copy, Paste, Format Painter | — (the assistant adds, moves and changes tasks directly: add_tasks, move_tasks, set_task_fields, format_tasks) |
| Font, Alignment | Font, size, bold, colours, alignment | ✅ format_tasks, format_sheet_cells |
| Tasks | New Task, Outdent, Indent, Information, Link, Unlink, Scroll to Task | ✅ add_tasks, outdent_tasks, indent_tasks, get_task, link_tasks, unlink_tasks, select_task |
| Editing | Find, Fill, Sort, Clear | ✅ get_tasks (search), set_task_fields, sort_view, set_task_fields |
| Proofing | Spelling | ✅ check_spelling |
| AI | AI Link | — (it is the link) |

## Schedule

| Group | Command | Tool |
|---|---|---|
| Insert | Task, Summary, Milestone, Recurring, Delete | ✅ add_tasks, indent_tasks, add_recurring_task, delete_tasks |
| Links | Link Tasks, Unlink, Relationships | ✅ link_tasks, unlink_tasks, get_task (links in and out) |
| Outline | Outdent, Indent, Outline Level, Move Up, Move Down | ✅ outdent_tasks, indent_tasks, outline_view, move_tasks |
| Tasks | Manually / Auto Schedule, Split, Inactivate | ✅ set_task_fields (mode, active), split_task |
| Properties | Information, Notes, Details, Add to Timeline | ✅ get_task, set_task_notes, show_panes, set_task_fields (timeline) |
| Paths | Critical Path, Longest Path, Task Path, Check Schedule | ✅ filter_view, get_tasks (critical), task_path, task_path_view, check_schedule |
| Change | Scroll to Task, Inspect, Global Change, Schedule Options | ✅ select_task, get_task; find_replace, set_schedule_options |

## Progress

| Group | Command | Tool |
|---|---|---|
| Update | Mark on Track, Update Tasks, Update Project | ✅ mark_on_track, update_progress, update_project |
| Period | Update Progress, Look-ahead, Status Date, Reporting Periods | ✅ update_project, lookahead, set_status_date, set_reporting_periods |
| Baselines | Set Baseline, Store Snapshot, Show Baseline, Tracking Gantt, Lock | ✅ set_baseline, take_snapshot, gantt_options, show_view, set_baseline_lock |
| Progress Reports | Progress Report, S-Curves, Manpower, Dashboard | ✅ period_report, progress_curve, resource_usage (trade), show_view |
| Measure | Rules of Credit | ✅ list_credit_rules, apply_credit_rule, define_credit_rule |

## Resource

| Group | Command | Tool |
|---|---|---|
| View | Resource Sheet, Team Planner | ✅ list_resources, show_view |
| Assignments | Assign Resources, Resource Pool, Resource Usage, Resource Graph | ✅ assign_resource, resource_usage, show_view, resource_pool, set_pool_plan, take_from_pool |
| Insert | Add Resource | ✅ add_resources |
| Level | Level All, Level Selection, Clear Leveling, Leveling Options | ✅ level_preview, level_resources, clear_levelling, set_schedule_options (levelling options) |

## Report

| Group | Command | Tool |
|---|---|---|
| View Reports | Dashboard, Progress Report, S-Curves, Report Builder | ✅ show_view, period_report, progress_curve, list_dashboards; Report Builder 🔜 5 |
| Health | Check Schedule, Risks, Schedule Risk, Delay Analysis, Milestone Trend | ✅ check_schedule, list_risks, schedule_risk, delay_analysis, milestone_trend |
| Portfolio | Portfolio, Compare Projects | ✅ portfolio, compare_plans |
| Export | Print / PDF, Page Setup, Export, Copy Picture | ✅ export_plan, export_register; Print and Page Setup 🔜 5; Copy Picture — |

## Project

| Group | Command | Tool |
|---|---|---|
| Properties | Project Information, Custom Fields, Activity Codes, Change Working Time, Pictures, Project Setup | ✅ set_project_info, define_custom_field, define_code, set_calendar, set_project_settings (start, calendar, hours per day, currency), set_task_expenses, set_task_documents, set_task_notebook, set_resource_rates, set_resource_availability, set_resource_leave, list_pictures, place_picture |
| Schedule | Set Baseline, Update Project, Reporting Periods, Check Schedule | ✅ set_baseline, update_project, check_schedule, set_reporting_periods, set_period_notes |
| Linked Plans | Link to Plan, Linked Plans, Update Links, Subplans, Apply Setup to Subplans | ✅ list_link_targets, link_to_plan, list_plan_links, update_links, list_subplans, new_subplan, add_subplan, remove_subplan, open_subplans, apply_setup_to_subplans |
| What-if | Scenarios | ✅ list_scenarios, new_scenario, compare_scenario, merge_scenario |
| Modules | Modules, Contract, Documents, Procurement | ✅ contract_status, set_contract…, document_register…, procurement_status…, set_modules |

## View

| Group | Command | Tool |
|---|---|---|
| Task and resource views | Gantt, Tracking Gantt, Network, Calendar, Relationship Diagram, Sheets, Task Usage, Timeline, Resource Sheet, Resource Usage, Team Planner | ✅ show_view |
| Data | Outline, Columns, Filter, Group, Sort, Tables, Highlight | ✅ outline_view, get_columns, set_columns, filter_view (highlight), group_view, sort_view, use_table |
| Zoom | Timescale, Zoom In, Zoom Out, Entire Project | ✅ zoom_view |
| Split View | Details, Timeline | ✅ show_panes |

## Format (Gantt Chart Tools)

| Group | Command | Tool |
|---|---|---|
| Format, Bar Styles, Gantt Chart Style | Bar styles, critical, slack, late, labels, baseline | ✅ gantt_options, set_bar_colours |
| Columns | Insert Column, Formula Column, Columns | ✅ set_columns, rename_column, define_custom_field (formula) |
| Conditional Formatting | Rules | ✅ set_conditional_formatting |
| Percent | Decimals | ✅ set_percent_decimals |

## Sheet Tools

| Command | Tool |
|---|---|
| Type in cells, formats, number formats, names | ✅ set_sheet_cells, format_sheet_cells, set_number_format, define_name |
| Add, rename, delete sheets | ✅ add_sheet, rename_sheet, delete_sheet |
| Insert and delete rows, columns and cells, move sheets, merge, freeze panes, conditional formatting, notes, hide, tab colour, row heights, sort, remove duplicates, pictures | ✅ insert_rows_columns, delete_rows_columns, shift_cells, move_sheet, merge_cells, freeze_panes, set_sheet_conditional_formatting, set_cell_notes, hide_rows_columns, set_sheet_tab, set_row_heights, sort_range, remove_duplicates, list_pictures, place_picture |

## File

| Command | Tool |
|---|---|
| New, Open, Save, Save As, Close | ✅ new_plan, list_templates, open_plan, save_plan, save_plan_as, close_plan |
| Import | ✅ import_plan (plan files; a table needs its columns matched in the window) |
| Export | ✅ export_plan |
| Print | 🔜 5 |
| Options (the app's own settings), Help, Check for Updates, GitHub | — |
