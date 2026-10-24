# Microsoft Project and Primavera P6 — Detailed Feature and Interface Reference

**Scope:** Functional reference for the specified desktop dialogs, views, configuration controls, and schedule-quality tools. The description distinguishes software behavior from broader implementation expectations and does not assume that similarly named controls behave identically across products. Exact labels, availability, and tab arrangements can vary by software version, edition, permissions, and database connection.

---

# PART A — MICROSOFT PROJECT (DESKTOP)

## A1. Task Information dialog — all principal tabs

The **Task Information** dialog is the detailed editing surface for a selected task. It should provide a single, consistent place to inspect the task's identity, scheduling attributes, resource assignments, dependencies, notes, and custom values. A user can ordinarily reach it by double-clicking a task or using a task-information command. Where a field is calculated rather than directly editable, the interface should identify that status and explain the scheduling rule responsible for its value.

### A1.1 General tab

**Purpose.** Capture the task's everyday planning and progress properties.

**Information and controls.** Task name; duration and duration units; start and finish dates; percentage complete; priority; the task's scheduling mode where supported (manually scheduled or automatically scheduled); and appropriate milestone or display-related options. Some controls vary with task type or desktop version. Summary task dates and durations should be rolled up from child tasks, rather than treated like ordinary independently scheduled activities.

**Expected behavior.** Editing duration, start, or finish must respect the selected scheduling mode, project calendar, task calendar, dependencies, and constraints. Automatic scheduling may alter dependent dates; manually scheduled entries may remain user-defined. The user should see a warning or explanation when an edit introduces a constraint, contradiction, or a potentially unexpected scheduling effect. Percentage complete should not be confused with percentage of work complete or physical progress.

### A1.2 Predecessors tab

**Purpose.** Define the relationships that connect a task to other tasks.

**Information and controls.** Predecessor task identifier and name; relationship type; and lead/lag. Supported dependency types are Finish-to-Start (FS), Start-to-Start (SS), Finish-to-Finish (FF), and Start-to-Finish (SF). Lag is a positive delay; lead is normally represented as a negative lag.

**Expected behavior.** Adding, removing, or modifying a predecessor updates the dependency network and invokes schedule recalculation when automatic scheduling is active. Cycles, self-references, deleted references, and malformed lag expressions must be rejected or clearly diagnosed. Relationship edits must remain consistent in the Gantt chart and network diagram.

### A1.3 Resources tab

**Purpose.** Assign resources to a task and control the assignment level.

**Information and controls.** Resource names; assigned units or allocation percentages for work resources; corresponding resource-specific assignment details; and the ability to add, modify, or remove assignments. Resource types can include work, material, and cost resources, with different applicable inputs.

**Expected behavior.** Changing units or adding resources can change work and/or duration depending on the task type and effort-driven setting. Resource assignments should be reflected in utilization views, assignment totals, estimated costs, and overallocation warnings. Assignment data should remain separate from resource master data.

### A1.4 Advanced tab

**Purpose.** Configure scheduling rules and task-specific attributes that are less frequently changed.

**Information and controls.** Constraint type and constraint date; deadline; task type (Fixed Units, Fixed Work, or Fixed Duration); effort-driven scheduling setting where applicable; task calendar and relevant calendar overrides; task inactivity where supported; and other version-specific advanced properties. A common distinction is that a deadline is a target/indicator, whereas a hard constraint can directly restrict calculated dates.

**Expected behavior.** Scheduling settings must have explicit and testable calculation consequences. Hard constraints can create unusual float or prevent an activity from shifting; deadlines should create warning indications without silently changing the basic logic of relationships. Fixed work, duration, and units choices must determine how the remaining variables change when an assignment is edited. Some advanced settings are unavailable for milestones, summary tasks, or certain task types.

### A1.5 Notes tab

**Purpose.** Retain human-readable information connected to the task.

**Information and controls.** Rich-text notes or comments, optional inserted objects or links depending on version and environment, and identifiable notes indicators in task views.

**Expected behavior.** Notes must be persisted with the task, searchable where practical, and remain distinct from scheduling data. Adding a note must not alter task dates or calculated values.

### A1.6 Custom Fields tab

**Purpose.** Display and edit user-defined task fields that are configured elsewhere.

**Information and controls.** Custom values such as Text, Number, Date, Flag, Cost, Duration, and other field types supported in the relevant desktop edition. The displayed set depends on defined fields and task context.

**Expected behavior.** A manually entered field permits compatible values; a formula-driven field should display its calculated result rather than accepting an entry that silently overwrites the formula. Fields using lookup lists must enforce their configured rules. Their values must be available in columns, filters, groups, reports, and indicators.

## A2. Field Settings dialog and table-column configuration

**Purpose.** Control how a column is displayed within a task or resource table, without confusing column presentation with the underlying meaning of the field.

**Controls.** Field selection where applicable; display title or alias; title alignment; data alignment; column width; header text wrapping; and related table-formatting choices. Context menus commonly expose operations such as Insert Column, Hide Column, and Field Settings.

**Behavior.** Changing a title or alignment must not rename or modify stored field values. Reordering, inserting, hiding, and resizing columns should be view-level operations. Saved views should restore those choices. Distinguish **Field Settings**, which primarily governs presentation, from **Custom Fields**, which governs data definition, lookup, formulas, and graphical indicators.

## A3. Custom Fields and the Lookup dialog

**Purpose.** Define structured user information beyond built-in schedule fields.

**Field definition.** Specify whether a custom field belongs to tasks, resources, or an applicable enterprise scope; choose its data type; assign a meaningful display name; select manual entry, lookup, or formula behavior; and define summary-row and assignment-row treatment where available.

**Lookup dialog.** Configure a table of permitted entries, including value and optional description; define ordering and default values where supported; and decide whether users must select a listed entry. Lookup values can represent stages, departments, disciplines, locations, priorities, or any other classification.

**Behavior.** A lookup is a validation and data-entry tool, not a scheduling formula. It should preserve data types, support controlled editing, prevent duplicate or invalid choices when configured, and be reusable by filter/group/report tools. Renaming a lookup label should not corrupt historical references. Imported values outside the lookup should be flagged rather than silently coerced.

## A4. Formula dialog

**Purpose.** Calculate a custom field value from existing values using a field-aware expression.

**Controls.** An expression editor; insertion menus for fields and functions; arithmetic, comparison, and logical operators; conditional expressions; data-type validation; and testing or validation feedback. Formula options and available functions depend on the custom field's type.

**Behavior.** Formulas evaluate deterministically and recalculate when their referenced inputs change. Expressions should distinguish dates, durations, numbers, money, Boolean values, and text. Errors such as division by zero, unknown field names, circular references, and incompatible types must be visible. Summary rows may use configured aggregation or a summary formula rather than automatically treating the parent as an ordinary task.

**Illustrative rules.** A numeric variance formula may subtract baseline duration from current duration. A status formula may return a code based on whether actual finish is later than baseline finish. Such examples must specify handling for incomplete tasks, absent baselines, and null values.

## A5. Graphical Indicators dialog

**Purpose.** Replace or supplement a custom field's displayed data with visual symbols driven by conditions.

**Controls.** Separate rules for non-summary rows, summary rows, and project summary where applicable; comparison operators (equal, not equal, greater than, less than, ranges, contains, and related options); thresholds; indicator image choices; and rule ordering.

**Behavior.** A rule engine checks the value against its configured criteria and displays the corresponding symbol or an explicit default. Multiple rules require well-defined priority. Indicators can show warnings, completions, blockers, and other states but do not change the underlying value. If a value cannot be calculated, a missing/error state should not be falsely shown as successful.

## A6. Bar Styles dialog

**Purpose.** Define how task categories appear in a Gantt chart.

**Style-rule list.** Each rule has a name, a task category or condition (such as normal, milestone, summary, critical, baseline, completed, or slack), a start field, a finish field, and a display order. The exact predefined styles vary by version and selected Gantt view.

**Bar appearance.** Format the bar's start marker, middle section, and end marker using shapes, patterns, thickness, and colors. A separate text area can place field values to the left, right, above, below, or inside a bar. Rules may apply to current-schedule bars, baseline bars, milestones, and float indicators.

**Behavior.** Style changes should immediately affect the Gantt view but never alter schedule calculations. Individual-bar formatting overrides should be clearly distinguishable from global style rules. If a baseline is absent, baseline bars should not fabricate dates. Legends used for printing should reflect applicable bar styles.

## A7. Timescale dialog

**Purpose.** Configure the horizontal time axis of the Gantt, usage, and other time-based views.

**Controls.** One, two, or three tiers (top, middle, bottom), where supported; time units from years to hours; interval counts; date-label formats; label alignment; tick lines; fiscal-year choice; tier separation; scale/zoom; and a live preview. Calendar views have additional week-heading and shading settings.

**Behavior.** The same underlying task dates remain unchanged when the timescale is altered. Zooming from months to days or hours changes the display resolution, not task duration. Nonworking time shading must use the relevant calendar. Timezone and daylight-saving boundaries require consistent date rendering. Date labels must remain legible at narrow zoom levels.

## A8. Page Setup and printing

**Purpose.** Control the production of readable, paginated schedules, diagrams, and reports.

**Page settings.** Paper size, portrait/landscape orientation, scaling or fit-to-page behavior, page numbering, print date/page ranges, and printer-related settings.

**Margins.** Top, bottom, left, and right margins, with appropriate handling of headers and footers.

**Header and footer.** Left, center, and right areas; configurable text; file name; current date/time; project data; page number; total pages; and optional graphics, subject to the selected view and edition.

**Legend.** Gantt and diagram legend content, placement on each page or a separate page, or suppression.

**View-specific print options.** Examples include gridline and note handling, page breaks, outline level, and calendar print-range options where supported.

**Behavior.** Print preview must represent the exported/printed result closely, including task columns, timescale, repeat headers, page breaks, and legends. Wide schedules should have clear multi-page or fit-to-width modes. Printing and exporting must not mutate saved schedule dates or progress.

## A9. Reports gallery and editing an open report

**Purpose.** Build presentation-ready summaries from current project data without manually rebuilding charts after each update.

**Reports gallery.** Provide predefined categories comparable to Dashboards, Resources, Costs, In Progress, and applicable Getting Started or custom reports. Examples include a project overview, cost overview, resource overview, overdue work, upcoming milestones, workload, and progress summaries. The exact gallery varies across desktop releases.

**Create report.** Support Blank, Chart, Table, and Comparison report types. A blank report begins as a canvas; a chart report begins with a visual; a table report begins with selectable field columns; a comparison report shows two side-by-side visuals.

**Report editor.** A selected chart or table exposes a field-selection pane for task, resource, or assignment data. Users choose measures and dimensions; filtering, sorting, grouping, and outline levels; chart type; labels; series; themes; fonts; colors; table formatting; titles; legends; images; shapes; and text boxes. Objects can be moved or resized on the page. Data changes should refresh visual results without destroying manual layout choices.

**Behavior.** Calculated metrics must identify their data source, unit, aggregation rule, and reporting/status date. Changes in report presentation must remain separate from changes in project data. Printing or sharing should preserve a readable page layout, while reports derived from unsupported or missing values should identify incomplete data rather than invent it.

## A10. Network Diagram view

**Purpose.** Present the activity-dependency network as connected boxes instead of a time-based Gantt chart.

**Contents.** One node per task or selected group; dependency arrows; critical/noncritical appearance; task identity; dates, duration, and float or other configurable fields. Node data templates control which fields appear within boxes.

**Controls.** Automatic layout; box alignment and spacing; row/column arrangement; manual node positioning; box and border styles; line styles (such as rectilinear and straight); arrows; link labels; zoom and print settings; grouping/filtering; and a legend.

**Behavior.** Layout changes do not rewrite task dependencies. Selecting a node should select the corresponding task. The diagram must identify predecessors and successors accurately, including their relationship type and lag when labels are enabled. Dense networks should remain navigable through zoom, pan, search, and selected-path highlighting.

## A11. Team Planner view

**Purpose.** Show tasks by assigned resource and permit visual workload adjustments. This is a Project Professional desktop feature, not universally available across editions.

**Layout.** One resource per row; scheduled assignment bars across a timeline; separate areas for unassigned work and assigned-but-unscheduled tasks. Progress and status are indicated through bar appearance. Overallocation conflicts are visually highlighted.

**Interaction.** Move an assignment horizontally to reschedule it, or vertically to reassign it; bring unassigned work onto a resource's row; inspect conflicting tasks; use resource-level filters and zoom; change display colors and row height.

**Behavior.** Moving assignments can affect schedule dates, assignments, work distribution, and leveling. A completed task or protected schedule item must not be silently moved. Multiple assignments and partially completed work require clear constraints. Overallocation should reflect resource capacity and concurrent work, not just overlapping graphics.

## A12. Resource Usage view

**Purpose.** Show resource totals and the detailed assignments contributing to them across time.

**Layout.** Hierarchical resource rows with assignment rows underneath; a left grid containing resource/assignment fields; and a right time-phased table of daily, weekly, or monthly values.

**Measures.** Work, baseline work, actual work, remaining work, overtime, cost, and relevant time-phased equivalents. Depending on settings, users can edit assignable time-phased work values, view utilization, and identify overallocations.

**Behavior.** A resource total should reconcile with its assignment data according to the selected measure and time interval. A change to time-phased work should update aggregate work and affected planning calculations as required. Users should be able to drill down, collapse/expand assignments, change displayed fields and units, and compare resource workloads.

## A13. View menu and view management

**Purpose.** Let users switch between representations of the same project data without recreating the project.

**Task views.** Gantt Chart, Tracking Gantt, Task Usage, Network Diagram, Calendar, Timeline, and other configured/custom task views.

**Resource views.** Resource Sheet, Resource Usage, Resource Graph, and Team Planner where licensed.

**View operations.** Choose task or resource views; change tables; apply filters and highlights; group or sort; set outline level; show/hide summary tasks; zoom or adjust timescale; open a split view or Details pane; create, rename, save, organize, or restore custom views where supported.

**Behavior.** The selected view determines which fields and visualization controls are relevant. A saved view should preserve its table, columns, group, sort, filter, layout, and scale as appropriate. Switching views never duplicates or modifies underlying tasks by itself. View-specific commands must be disabled or explained when not applicable.

## A14. Timeline and Timeline Format controls

**Purpose.** Show selected milestones and high-level activities in a compact presentation timeline.

**Contents and controls.** Add/remove tasks or milestones; one or more timeline bars in supported versions; date-range selection; milestone symbols; tasks displayed as bars or callouts; adjustable rows and label placement; font, color, background, and date-label formatting; optional progress shading; and timeline copying/printing.

**Behavior.** The timeline is a curated summary, not a separate schedule. Changes to a task's scheduled dates should propagate to its timeline representation. Repositioning labels or moving a callout should not change the underlying schedule. Date-range adjustments must clip or hide out-of-range activities predictably without deleting them.

---

# PART B — ORACLE PRIMAVERA P6 PROFESSIONAL

## B1. Activities window and bottom Activity Details tabs

The **Activities** window is the central activity-editing environment. The top portion can contain an Activity Table, Gantt chart, Activity Network, or other selected representation. A lower pane contains detailed information for the selected activity. A selected activity remains the same record whether edited in the table, chart, or detail panel.

**Core activity fields.** Activity ID and name; WBS; calendar; type; duration type; percentage-complete type; start/finish information; planned, actual, remaining, and at-completion durations; constraints; float; activity codes; role/resource assignments; and predecessor/successor relationships.

**Activity types.** Task Dependent, Resource Dependent, Level of Effort, Start Milestone, Finish Milestone, and WBS Summary. Task Dependent activities typically use an activity calendar; Resource Dependent activities can use assigned resource calendars; Level of Effort activities derive their duration from driving relationships; milestones have zero duration; WBS Summary activities summarize work associated with the WBS.

**Duration types.** Fixed Units/Time, Fixed Duration & Units/Time, Fixed Units, and Fixed Duration & Units. These govern which variables are held constant when duration, planned effort, or resource productivity changes. **Percentage-complete types** include Duration, Units, and Physical, and must not be treated as interchangeable calculations.

### B1.1 General tab

Edit or review activity type, duration type, percentage-complete type, calendar, assigned WBS, responsible manager, primary resource, and other general administrative properties. Changes to these fields may affect which scheduling calendar or work/duration calculation applies.

### B1.2 Status tab

Record actual start, actual finish, activity status, planned/original duration, actual duration, remaining duration, at-completion duration, remaining/early dates, expected finish, float, percentage complete, and applicable cost or unit totals. Configure primary/secondary constraints and their dates; in applicable cases capture suspend and resume dates. Status edits must be interpreted relative to the project's data date.

### B1.3 Resources tab

Assign labor, nonlabor, and material resources or roles; review planned, actual, remaining, and at-completion units/costs; manage units per time, rates, and role substitutions where supported; and identify the resource assignment responsible for driving dates. A resource assignment is not the same as an activity code or activity type.

### B1.4 Predecessors tab

Show each preceding activity, its project/activity identity, relationship type (FS, SS, FF, SF), and lag. Support add, remove, navigate-to, and edit actions. Cross-project relationships must remain identifiable.

### B1.5 Successors tab

Display corresponding outgoing relationships to successors, with the same relationship types and lag parameters. Editing either predecessor or successor information must update the same underlying relationship record.

### B1.6 Relationships tab

Present incoming and outgoing links together to help users inspect the activity's dependency context. Relationship modifications should reconcile across the table, Gantt, predecessor, and successor panels.

### B1.7 Codes tab

Assign one value for each applicable activity-code definition. Display code definition, selected value, and description; support search, assignment, removal, and constrained value creation. Only codes visible to the current project/scope and authorized user should be selectable.

### B1.8 Steps tab

Break an activity into smaller checkable steps without necessarily creating separate CPM activities. Each step can have a description, status/completion, and an optional weight; weighted steps can support measurement of physical completion when configured. Step completion and activity completion should have explicit relationships rather than being automatically equated in all cases.

### B1.9 Notebook tab

Maintain structured explanatory notes organized by notebook topic. This is suited to assumptions, scope clarifications, construction/operational notes, and decisions. Notebook text should remain searchable and separate from network logic.

### B1.10 Expenses tab

Enter nonresource expense items related to an activity, including expense category, supplier/vendor where applicable, planned/budgeted cost, actual cost, remaining cost, and accrual/distribution settings. Expenses must reconcile with activity cost reporting but must not be double-counted as labor or equipment units.

### B1.11 WPs & Docs tab

Associate work products, reference documents, deliverables, and links with activities. The relationship can include a document reference and deliverable state. File links must preserve identity and access rules independently of scheduling fields.

### B1.12 UDFs tab

Display and edit User Defined Fields according to their defined data types, scope, and permissions. UDFs should participate in table-column selection, filtering, grouping, and reporting without automatically becoming CPM calculations.

### B1.13 Summary tab

Display calculated and rolled-up schedule or assignment information useful for reviewing the selected activity. Values should identify the calculation basis and should not be mistaken for manually entered underlying values.

### B1.14 Additional or environment-dependent tabs

Other tabs can include Risks, Tasks, Discussion, and Feedback. Their availability depends on P6 Professional versus EPPM connections, database configuration, and access rights. Risks link activity records to identified project risks; Tasks/Discussion/Feedback provide ancillary collaboration or assignment-workflow information where enabled. The lower detail panel supports customizing which tabs are visible and their order; do not assume every installation has every tab.

## B2. Layout options and saved activity layouts

**Purpose.** Save and restore an entire view configuration, not just a column selection.

**Available layout content.** Top and bottom panes; Activity Table plus Gantt chart; Activity Network; Trace Logic; Activity Details; resource/activity usage profiles and spreadsheets; and related display combinations supported by the installation.

**Columns.** Add, remove, reorder, resize, freeze, or select visible activity fields. A column configuration must reference the underlying field identity rather than only its caption.

**Group and Sort.** Organize activities by WBS, activity code, calendar, responsibility, date, status, and other attributes. Show grouping bands and summaries; use multiple sort priorities; specify hierarchical ordering where relevant.

**Filters.** Include and exclude activities using named/custom expressions with field, operator, and value; combine multiple criteria; optionally select activities by WBS or status. Filtering must hide rows in the view rather than deleting activities.

**Gantt formatting.** Choose bar definitions and labels, relationship lines, timescale, date markers, nonworking time, activity progress, baseline comparisons, colors and text arrangements.

**Layout lifecycle.** Open, preview/apply, save, save as, rename, import/export where supported, restore defaults, and control visibility/availability (user, project, or wider access according to permissions). An unsaved-changes prompt should differentiate temporary display edits from saved layout modifications.

**Behavior.** A saved layout must reproduce all applicable display settings when reopened. Layout changes must not recalculate or overwrite task dates. Data edits made through a layout, however, act on the underlying activity and may affect the schedule.

## B3. Activity Codes — definitions, values, scopes, and assignments

**Purpose.** Provide controlled classifications independent of WBS hierarchy. WBS answers *where an activity sits in the breakdown*; activity codes answer *which categories the activity belongs to*.

**Scope levels.** **Global** codes apply across the enterprise; **EPS** codes apply to an enterprise-project-structure branch and eligible descendants; **Project** codes apply to a specific project. The chosen scope controls availability and access.

**Code definition.** Code name, optional code-length constraints, ordering, administrative ownership, and available values. Values include a code and description; hierarchies may be used where supported. An activity is generally assigned one value per code definition, but it may be assigned values from several different codes.

**Typical examples.** Discipline (Mechanical, Electrical, Civil), Location (Area A, Area B), Phase (Design, Procurement, Test), Responsible Team, Contract Package, and Priority Class. These are examples of data, not fixed built-in categories.

**Uses.** Codes can drive filters, group bands, sorting, charts, report segmentation, and bar coloring in the appropriate visualizer. Renaming values should preserve assignments by internal identity. Removing a code or changing its scope must warn about affected activity records and saved layouts.

## B4. Schedule Options — General and Advanced

The **Schedule Options** dialog governs how a CPM calculation is carried out. These settings are calculation policies; they must not be treated as cosmetic view preferences. The schedule should record its data date, policy selections, run status, and calculation log so that results can be reproduced.

### B4.1 General: scheduling behavior

- **Ignore relationships to/from other projects:** decide whether cross-project links are included when scheduling the open project set. The choice changes the network boundaries and may change calculated dates.
- **Make open-ended activities critical:** optionally mark activities lacking a predecessor or successor as critical for review. This is a diagnostic designation, not proof that the activity belongs to a driving longest path.
- **Use Expected Finish Dates:** include eligible user-entered expected finishes in calculation where configured.
- **Schedule automatically when a change affects dates:** choose automatic recalculation after significant changes versus explicit scheduling runs.
- **Level resources during scheduling:** optionally invoke resource leveling as part of a scheduling run; leveling should be documented separately from pure logic-based CPM.
- **Recalculate assignment costs after scheduling:** update costs when new dates cause time-based rate changes.

### B4.2 General: out-of-sequence progress handling

Out-of-sequence progress occurs when a successor has begun or progressed despite a logical predecessor not satisfying its relationship.

- **Retained Logic:** holds the remaining portion of the progressed successor until the outstanding predecessor logic is satisfied.
- **Progress Override:** permits remaining work to proceed without respecting the unsatisfied predecessor logic for the out-of-sequence relationship.
- **Actual Dates:** uses actual dates in forward/backward scheduling calculations, with consequences that can include unusual or negative float when actuals and data date conflict.

These choices can produce different forecasts from identical task and progress data. The selected approach must be visible alongside schedule results.

### B4.3 General: lag and criticality calculations

- **Start-to-Start lag reference:** when a predecessor started out of sequence, calculate eligible SS lag from Actual Start or Early Start, according to the selected policy.
- **Critical activity definition:** either a Total Float threshold (for example, float less than or equal to a chosen duration) or **Longest Path** based on driving logic.
- **Float end-date reference:** calculate float against each project finish or the collective finish of opened projects, depending on the project set and policy.
- **Total Float convention:** compute using start float, finish float, or the smaller of those measures, according to configuration.
- **Relationship lag calendar:** select predecessor calendar, successor calendar, a 24-hour calendar, or project default calendar, as supported. A lag of three working days is not necessarily 72 calendar hours.

### B4.4 Advanced: multiple float paths

**Calculate multiple float paths** identifies the most critical path and additional near-critical paths. Options include path-calculation basis (Total Float or Free Float, where the latter is associated with longest-path tracing), target ending activity, and the number of paths to calculate. Results can be exposed through Float Path and Float Path Order fields for grouping and sorting.

**Behavior.** Schedule output must distinguish total float from free float, and distinguish a float-threshold critical activity from a driving-relationship longest-path activity. Calendar differences, constraints, cross-project links, and progress policies can cause these measures to differ. A calculation should issue diagnostics when the network contains invalid links, cycles, missing references, impossible constraints, or contradictory actual dates.

## B5. Check Schedule — configurable quality analysis

**Purpose.** Assess schedule integrity using configurable checks and thresholds. This is an analytical and diagnostic workflow that is separate from the act of calculating the CPM dates. The check produces a report of passed/failed tests, affected activities or relationships, and suggested investigation areas; failing a heuristic is not automatically proof of an incorrect schedule.

### B5.1 Relationships and Assignments checks

1. **Logic:** missing predecessors or successors, with sensible exclusions for project starts, ends, milestones, completed activities, or nonstandard activity types.
2. **Negative Lags:** relationships with lead (lag below zero).
3. **Positive Lags:** relationships with a delay above zero.
4. **Long Lags:** lag duration exceeds a configured threshold, accounting for working-time conversion.
5. **Relationship Types:** percentage of relationships that are Finish-to-Start versus other types.
6. **Out-of-Sequence:** successors progressing despite unsatisfied logic.
7. **Resource/Cost:** applicable activities without an assigned resource, role, or expense.
8. **Dangling Start:** activities whose start is not driven by predecessor logic.
9. **Dangling Finish:** activities whose finish does not drive successor logic.

### B5.2 Dates and Durations checks

10. **Hard Constraints:** counts or percentages of mandatory start/finish restrictions.
11. **Soft Constraints:** counts or percentages of other date constraints that may distort network flexibility.
12. **Large Float:** total float exceeds a configurable maximum.
13. **Negative Float:** total float below zero, signaling schedule pressure or constraint/actual-date conflict.
14. **Large Durations:** remaining duration exceeds a configured upper limit.
15. **Invalid Progress Dates:** actual dates beyond the data date or unfinished work incorrectly positioned before it.
16. **Late Activities:** forecast/current finish later than the corresponding approved baseline finish.
17. **Baseline Integrity:** activities missing from an assigned baseline or missing a valid baseline mapping.
18. **Links to Closed Projects:** cross-project predecessor/successor dependencies whose projects were not included in the scheduling session.
19. **Baseline Execution Index (BEI):** completed qualifying activities divided by qualifying baseline activities scheduled to be complete by the data date, with documented exclusions and handling of a zero denominator.

The application and software versions may distribute these checks across different tabs or menus. Oracle documentation includes both Relationships and Assignments and Dates and Durations categories, as well as additional project-context reporting.

### B5.3 Thresholds and interpretation

Every check should identify its numerator, denominator, exclusions, threshold, test condition, and severity. For example, a target written as **less than 5%** means the result fails when the measured share is 5% or above; a target such as **BEI at least 0.95** uses a different direction. Defaults are guidelines rather than universal contractual limits and should be editable. The report must clearly separate percentages of *activities* from percentages of *relationships*.

### B5.4 Report and drill-down behavior

The check result should identify project, data date, total activities, scheduled/completed counts, relationship counts, selected thresholds, run date, and version of scheduling options. A summary shows passed/failed/not-applicable tests. Each failed test should expose the affected activity IDs, relationship IDs or links, measured value, and reason. Users should be able to filter the Activities view to failed items, inspect the issue, correct the data, rerun the checks, and export the report. Project settings, selected thresholds, and results should be saved or reproducible for audit comparison.

---

# PART C — CROSS-FUNCTIONAL BEHAVIOR AND VALIDATION

## C1. Shared behavioral principles

1. **Single source of project data.** Task/activity tables, information dialogs, Gantt views, networks, timelines, assignments, and reports must refer to the same records and stable IDs.
2. **Separate data, calculation, and presentation.** Changing a bar color, column width, report design, or layout must not change scheduled dates. Editing a dependency or scheduling policy may change computed dates.
3. **Explicit calculated versus entered values.** The interface must visibly distinguish user-entered inputs, calculation outputs, baseline snapshots, and historical actuals.
4. **Validation and traceability.** Invalid references, circular dependencies, inconsistent actuals, impossible dates, unsupported field types, and formula errors require actionable diagnostics rather than silent correction.
5. **Consistent units.** Time, working-time calendars, money, percentages, resource units, and durations must retain their meaning across dialogs and exports.
6. **Reversible edits.** Multi-field or multi-row edits should be undoable where possible, with prompts before destructive actions.
7. **Saved customization.** Custom fields, lookups, indicator rules, layout choices, report templates, print formats, and view configurations should survive save/reopen and be versioned or migratable.
8. **Reproducible outputs.** A printed view or schedule-check report should identify the underlying data date, baseline, and configuration needed to interpret it.

## C2. Practical acceptance scenarios

- Edit a task's FS predecessor to SS with a two-working-day lag and verify that the task's scheduled dates, Gantt link, and network link all change consistently.
- Change a custom lookup value, verify that task-level validation and report/group filters use the same defined value, and verify that historical assignments are not lost during a label rename.
- Create a formula-driven field, test null values, invalid types, summary roll-up behavior, and the graphical-indicator thresholds that consume its result.
- Save two different Gantt bar-style and timescale configurations, reopen each, and verify that no activity dates changed.
- Edit a resource assignment in Team Planner, then verify reconciliation in Resource Usage and conflict indicators.
- Create a report with a chart and table, change activity progress, and confirm both refresh from the same project data without losing layout formatting.
- Compare scheduling the same progressed network under Retained Logic and Progress Override. Record and explain the resulting dates and float instead of expecting both policies to produce identical outputs.
- Run schedule checks with a default and custom threshold, inspect failed activities, repair the issue, rerun, and compare the reports.
- Export a wide Gantt view with a multi-tier timescale and a selected date range. Verify the preview, actual print pagination, legends, margins, and readable headers.

---

# REFERENCE SOURCES

The following are official product references used to verify terminology and product behavior. Product editions and releases differ; each source should be matched to the target version when an exact UI reproduction is required.

**Microsoft Support**

- Create a custom field: https://support.microsoft.com/en-us/project/create-a-custom-field-in-project
- Change the timescale: https://support.microsoft.com/en-us/project/change-the-timescale-in-a-project-view-in-project-desktop
- Format Gantt bars: https://support.microsoft.com/en-us/project/format-the-bar-chart-of-a-gantt-chart-view
- Create a Project report: https://support.microsoft.com/en-us/project/create-a-project-report
- Network Diagram: https://support.microsoft.com/en-us/project/create-a-network-diagram-in-project-desktop
- Team Planner: https://support.microsoft.com/en-us/project/view-your-team-s-work-with-team-planner
- Resource progress and usage: https://support.microsoft.com/en-us/project/review-the-progress-of-your-schedule
- Create and format a timeline: https://support.microsoft.com/en-us/project/create-a-timeline-in-project
- Print and page setup: https://support.microsoft.com/en-us/project/print-a-project-schedule-in-project-desktop

**Oracle Primavera P6 Professional Help**

- Activity Details: https://docs.oracle.com/cd/F88968_01/client_help/en_US/activity_details.htm
- Activities window: https://docs.oracle.com/cd/F51303_01/client_help/en_US/activities_window.htm
- Activity types and duration types: https://docs.oracle.com/cd/E90748_01/client_help/en_US/general_tab_-_activity_details.htm
- Activity codes: https://docs.oracle.com/cd/F88968_01/client_help/en_US/activity_codes_and_values.htm
- Scheduling options (General): https://docs.oracle.com/cd/F51303_01/client_help/en_US/general_tab_-_schedule_options_dialog_box.htm
- Scheduling options (Advanced): https://docs.oracle.com/cd/F25600_01/client_help/en_US/advanced_tab_-_schedule_options_dialog_box.htm
- Check Schedule overview: https://docs.oracle.com/cd/G48902_01/client_help/en_US/101393.htm
- Check Schedule relationships: https://docs.oracle.com/cd/G48902_01/client_help/en_US/113174.htm
- Check Schedule dates and durations: https://docs.oracle.com/cd/G48902_01/client_help/en_US/113175.htm
