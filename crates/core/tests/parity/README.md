# Parity plans

Each file here is a plan that carries its own dates: `Start`/`Finish` in a Project XML file, `early_start_date`/`early_end_date` in an XER file. The test `parity::files_keep_their_dates` opens every file, schedules it with Datum Plan's engine, and checks that every task gets the same start and finish (to the minute) as the file says.

- `network.xml` and `network.xer` are made by hand: a small network with every common rule (a lag, a start-to-start link, a holiday, a milestone). Their dates were worked out by hand with the standard critical-path rules, as in `src/cases.rs`.
- **To check against another scheduling tool, export a plan from it (XML or XER) and drop the file in this folder.** If a date differs, the test lists the task, the file's date and ours. Keep files small and free of private names.
