# The Datum family: one design and one way of working

The Datum apps are small, fast Windows desktop tools that make engineering and science work easier. **Datum Plan** (scheduling: MS Project + Primavera P6) is the first. **Datum Plot** (graphing and statistics: GraphPad Prism + OriginPro) is next. Every app looks, feels and ships the same way; only the accent colour and the logo glyph change.

This file is the single source for that. Datum Plan's code is the reference implementation: copy from it rather than reinventing anything.

## 1. The apps

| App | What it replaces | Accent | Logo glyph | Files | Identifier | Repo |
| --- | --- | --- | --- | --- | --- | --- |
| Datum Plan | MS Project + Primavera P6 | Blue `#0B5CAD` (glyph `#2F6FCF`) | Calendar with bars + amber diamond | `.dplan` | `com.edgarmendonca.datumplan` | Edgar-Mendonca/Datum-Plan |
| Datum Plot | GraphPad Prism + OriginPro | Teal `#0E8C7F` (glyph `#0E8C7F`) | Axes, points, fitted curve + amber diamond | `.dplot` | `com.edgarmendonca.datumplot` | Edgar-Mendonca/Datum-Plot |
| Next apps | — | pick from §3 | — | `.d<name>` | `com.edgarmendonca.datum<name>` | Edgar-Mendonca/Datum-<Name> |

## 2. Stack (the same in every app)

- **Rust + Tauri 2** desktop shell (`src-tauri`), Windows first (NSIS per-user installer, WebView2).
- **Engine in a pure-Rust crate** (`crates/core`): all calculation lives there, with unit tests. The window only shows what the engine computes.
- **UI: SolidJS + Vite + strict TypeScript** in `ui/src/*.tsx`, built to `ui/dist` (embedded at compile time). State in `ui/src/store.ts`. Plain CSS with tokens (no CSS framework).
- **Charts:** Chart.js in a Prism style (see §7). Datum Plot may add its own SVG plot engine for publication graphs, but must keep this look.
- **Excel formulas:** the GridCraft crates (pinned git dependency, attribution in NOTICE) where the app needs them.
- **No Python** inside the apps (size and second runtime).
- Small, compact code that matches the surrounding style.

## 3. Colour

### Base tokens (all apps, from `ui/src/styles.css`)
Light:

```
--bg #F2F3EF  --bg2 #FFFFFF  --panel #F8F8F5  --panel2 #ECEDE8  --side #F3F4F0
--line #D3D7D6  --line2 #E6E8E5  --line3 #B9BFC2
--text #0C1A2B  --text2 #3B4859  --muted #5B6878  --navy #0C1A2B
--amber #B7791F  --crit #C0392B  --green #1F7A4D
--ok-bg #DDF2E5 --ok-fg #1F7A4D  --warn-bg #FBEFD5 --warn-fg #8A5A00  --err-bg #FBE3E0 --err-fg #9B2C2C
```
Dark (`:root[data-theme="dark"]`): `--bg #0B1119 --bg2 #101823 --panel #0D141E --panel2 #16212E --line #223041 --text #E6EDF5 --muted #8D9AAB` (copy the full block from Plan).

### The accent set (the only tokens that change per app)
`--blue` (accent), `--blue2` (hover/dark), `--bar`, `--sky` (light tint), `--pale` (very light), `--sel`, `--sel2` (selection), and the File sidebar colour.

| Accent | `--blue` | `--blue2` | `--sky` | `--pale` | `--sel` | `--sel2` | dark `--blue` | File sidebar (dark) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Plan · Blue** | #0B5CAD | #0A4F96 | #9CC2F7 | #E8F0FA | #DCE8F7 | #B9D2F0 | #4C9BEA | #0A3D73 |
| **Plot · Teal** | #0E8C7F | #0B7468 | #8FD3C9 | #E3F5F2 | #D3EFEA | #A9DDD4 | #3FC2B2 | #0A4A44 |
| Violet (spare) | #6C4AB6 | #5A3C9C | #C9B8EE | #F0EBFA | #E5DCF6 | #CDBDEE | #A58BEA | #3A2766 |
| Amber (spare) | #A8650F | #8E550C | #F2CF8E | #FBF2E2 | #F6E7CB | #EBD09E | #F0B450 | #5A3A0C |
| Crimson (spare) | #B83250 | #9C2944 | #F1B3C1 | #FBEBEF | #F6DCE3 | #EDBAC7 | #EF7A93 | #5E1A2A |

Keep `--crit` red, `--green` and `--amber` for meaning (critical/error, done/ok, warning/highlight) in every app, whatever the accent.

## 4. Type

- **IBM Plex Sans** for text, **IBM Plex Mono** for uppercase spaced labels, tabs, buttons, numbers and IDs (`@fontsource`, Latin subsets only, about 200 kB).
- Group captions, card headings and buttons: Plex Mono, 9.5–11 px, `letter-spacing: .1em`, uppercase.
- Body 12–13 px; page titles 23 px, 600–700 weight.
- Tables and grids: 12–13 px; numbers right-aligned.

## 5. Logo and branding

- **Tile:** 256×256 viewBox, a rounded square (`x=8 y=8 w=240 h=240 rx=56`), a white→accent-tint diagonal gradient, and a 4 px border in a light accent tint.
- **Glyph:** drawn in the accent with **11 px strokes**, round caps. It may use one light tint fill (the `--sky` colour). It always has **one amber `#F2B233` diamond** (the family mark).
- Files: `ui/public/logo.svg` (in the app) and `docs/brand/datum-<app>-icon.svg` (README). Windows icons are generated from it (`npx tauri icon`).
- References: Plan `docs/brand/datum-plan-icon.svg`; Plot draft `docs/brand/datum-plot-icon.svg` (teal axes, tint data points, fitted curve, amber diamond).
- **Name:** "Datum <App>" in Plex Sans 600. Tagline in the README.
- **README:** the logo, the name, a one-line tagline, shields.io badges (release, CI, downloads, license MIT, platform Windows 10/11, Rust, Tauri 2, SolidJS), then for-the-badge buttons (Download for Windows · What's new · Memory & roadmap), then a screenshot.

## 6. Window layout

- **Title bar:** quick access (save, undo, redo, print), document name, "Tell me what you want to do" (Ctrl+K), a Theme button (Auto/Light/Dark).
- **Ribbon (MS Office style):** a blue FILE button, then tabs. Groups have a mono uppercase caption. Big buttons have an icon above the label; split buttons have the command on top and the menu on the bottom ▾. Contextual tabs carry a small label above them. The ribbon never scrolls sideways: it tightens, then folds groups from the right (icons only).
- **Left sidebar (Cairn style):** icon + mono label per view, with **Setup** first. Groups are separated by rules.
- **File page (backstage):** a sidebar in the accent colour (Home, New, Open, Import, Save, Save As, Export, Print; Options and Updates at the bottom).
  - Home: template tiles, recent files, and the updates card. The card has Check again with a spinner and the time it checked, and the GitHub, Releases and Report an Issue buttons.
- **Status bar:** Ready, key facts, view switches, zoom.
- **Options page:** sections (General, Start-up and saving, New <documents>, Printing and branding, Updates). Each option has a name plus a one-line hint on the left and the control on the right.
- **Help tab:**
  - Getting Started: written for people who know the tools being replaced;
  - Glossary: plain-language terms, searchable;
  - Shortcuts, Tell Me;
  - the GitHub links.

## 7. Components

- **Controls:** square corners, 1 px lines, no heavy shadows. Buttons are Plex Mono uppercase (`.btn`, `.btn.pri` filled accent, `.btn.sm`).
- **Menus:** icons in the tick column, keyboard hints on the right, submenus to any depth, close on an outside click.
- **Dialogs:** a title bar with ×; OK/Cancel at the bottom right; Enter = OK, Esc = Cancel.
- **Grids (Excel-like):**
  - one scroller with a sticky header and frozen key columns;
  - range selection (drag, Shift, Ctrl, whole rows and columns, Ctrl+A);
  - Sum/Average/Count in the formula bar;
  - TSV clipboard with Excel;
  - a fill handle with series (Ctrl copies);
  - Paste Special (Ctrl+Alt+V);
  - Font/Alignment formatting (Ctrl+B/I/U);
  - a date picker on date cells.

  Reuse Plan's `selection.ts`, `cells.ts` and `fmt.ts`.
- **Toasts:** bottom left; green for success.
- **Empty states:** say what the view is for and offer the first action.

### Icons
24×24 viewBox, in one SVG sprite in `ui/index.html` (`<use href="#i-name">`).
- Line style: `stroke="currentColor"`, `stroke-width 1.6–1.8`, round caps and joins.
- At most one light accent fill (`#9CC2F7` in Plan; use the app's `--sky`).
- Red `#C0392B` only for delete or critical; green `#2E9D57` for done or ok; amber for highlight.
- One icon per meaning (no duplicates).

### Charts (Prism style)
Chart.js:
- white plot area, thin axes, no heavy gridlines;
- Okabe-Ito colour-blind-safe series colours (`#0072B2 #E69F00 #009E73 #D55E00 #CC79A7 #56B4E9 #F0E442 #000000`);
- a crosshair tooltip;
- a PNG button on every chart; charts printable.

## 8. Printing

- File › Print (Ctrl+P) shows settings on the left and a live page preview on the right, for **every view**.
- The settings: paper, orientation, scaling or fit-to-pages, margins, page ranges, header and footer (left/centre/right) with codes and logos, legend, and view options.
- Pages are drawn as SVG at real size and printed with the WebView dialog ("Save as PDF").
- Report-type pages print as flowing documents with the header and footer on every page and page numbers.
- Reuse Plan's `print.tsx` (`Frame`, `Slot`, codes).

## 9. Updates and releases

- **Updater:** `src-tauri/src/update.rs`.
  - Set `REPO` to the app's repo. It checks GitHub releases at start, and beta versions when turned on.
  - It downloads the installer, checks SHA256SUMS and installs.
  - The CHANGELOG section becomes the in-app "What's new".
- **Versions:** small numbered releases (0.1.0, 0.1.1, …), always three numbers (the updater compares three).
- **Release workflow** (`.github/workflows/release.yml`, `windows-latest`):
  - builds the UI and the app and creates the tag;
  - publishes `Datum<App>-x.y.z-setup.exe` + `SHA256SUMS.txt`.
  - Run it with workflow_dispatch when tag pushes are blocked.
- **CI** (`ci.yml`, ubuntu): clippy `-D warnings`, `cargo test --workspace`, `npm run check:ui`, the Playwright UI tests.
- Options and recovery files in the app's data folder (`com.edgarmendonca.datum<app>`).

## 10. Docs in every repo

- `CLAUDE.md`: read MEMORY first; update it at the end of every session; the user's preferences; the checks; the release steps.
- `docs/MEMORY.md`: product table, architecture (files), decisions (version | decision | why), roadmap (✅ 🔜 ⬜), session log, known limits, and the "How we work" workflow.
- `CHANGELOG.md`: `## x.y.z — date`, a one-line summary, then bold sections in plain language for users.
- `docs/PLAN-x.y.md`: feedback → diagnosis → proposal → size → questions, written before any big release.
- `NOTICE` for third-party code and fonts; MIT license.

## 11. How we work (from Datum Plan)

1. The user reviews a release and sends screenshots.
2. Reproduce first.
3. Big asks get a plan file and questions; build after the answers.
4. Engine first (with a test), then the UI.
5. Check every change in the real app (Xvfb screenshots).
6. Add a UI test for every bug.
7. Run all checks.
8. Release.
9. Report in plain language: what changed, what was checked, what is not done.

## 12. Starting a new Datum app (checklist)

1. Create the repo `Edgar-Mendonca/Datum-<Name>` (MIT).
2. Copy the skeleton from Datum Plan:
   - the workspace `Cargo.toml`, `crates/core` (empty engine), `src-tauri` (main, update.rs, tauri.conf);
   - `ui/` (index.html with the icon sprite, styles.css, store.ts, ui.tsx, ribbon/backstage/options/print patterns);
   - `package.json`, `.github/workflows`, `tests_ui/run.cjs`, `CLAUDE.md`, NOTICE, LICENSE.
3. Rename: product name, identifier, file extension, `REPO` in update.rs, installer name in release.yml.
4. Apply the accent set from §3 and draw the logo per §5.
5. Write `docs/MEMORY.md` and `docs/PLAN-0.1.md` (brainstorm and questions) before building features.
6. Release 0.1.0 (shell, updater, Home, one working view) so the updater works from day one.
