# DATUM: the family hub (brief)

The repo **Edgar-Mendonca/DATUM** is the home of the Datum family. It holds two things:

1. **The brain.** How every Datum app is designed, built and released: the shared design guide, the family memory, and the list of apps. An app repo keeps its own `docs/MEMORY.md`; DATUM keeps what is common to all of them.
2. **The website.** A small static site on GitHub Pages (built with Astro). It shows each app, what it is for, what it does today, and what comes next. It reads that information from the app repos when it builds, so it never goes stale.

## 1. Positioning (how we talk about Datum)

> **Datum: free, open-source desktop tools for engineers, scientists, researchers and analysts.**
> Small, fast apps that do serious technical work, written from scratch in Rust, with one look and one way of working. No accounts, no subscriptions, no telemetry: your files stay on your computer.

Rules for all public text (README, site, release notes):
- Describe what each app does and who it helps. **Never name or compare with commercial products**, and never say an app copies, clones or replaces one. Every Datum app has its own engine and its own design.
- Say "free and open source (MIT)", "Windows first", "installs in seconds", "updates itself".
- Write plainly: short sentences, active verbs, no marketing filler. A visitor should know in ten seconds what each app is for.

The apps, in public words:

| App | One line | For | Accent |
| --- | --- | --- | --- |
| **Datum Plan** | Plan, schedule, track and report projects. | Project managers, planners, engineers, contractors | Blue `#0B5CAD` |
| **Datum Plot** | Analyse data and make publication-ready graphs. | Scientists, researchers, lab teams, students | Teal `#0E8C7F` |
| **Datum Atlas** | Choose materials with property charts and selection indices. | Design and materials engineers, students | Violet `#6C4AB6` |

The family mark uses the shared tile (256 px, rx 56, light gradient, 11 px strokes) with one amber diamond `#F2B233`; use a neutral navy `#0C1A2B` glyph (a "D" whose bowl holds the three accent dots) for DATUM itself.

## 2. What goes in the repo

```
DATUM/
  README.md                 the family front page (see §3)
  FAMILY.md                 shared design guide (copied from Datum-Plan/docs/FAMILY.md, then owned here)
  MEMORY.md                 family memory: decisions that apply to all apps, the apps list, session log
  apps.json                 the registry: one entry per app (see §5)
  brand/                    datum-icon.svg + each app icon (copied from the app repos)
  site/                     the Astro site
    astro.config.mjs        site: https://edgar-mendonca.github.io, base: /DATUM
    package.json
    src/lib/github.ts       build-time fetch from the app repos
    src/styles/tokens.css   the family tokens (from FAMILY.md §3)
    src/layouts/Base.astro
    src/components/         AppCard, ReleaseBadge, Roadmap, Feature, Shot, Header, Footer
    src/pages/index.astro   home
    src/pages/[app].astro   one page per app (plan, plot, atlas)
    src/pages/family.astro  design and principles
    src/pages/about.astro   about, contribute, licence
  .github/workflows/pages.yml
```

`FAMILY.md` becomes the source of truth in DATUM. App repos keep a short `docs/FAMILY.md` that links to it (or a copy synced on release).

## 3. README (DATUM)

- Centered family logo, title **Datum**, the positioning line, badges (site, licence MIT, "free & open source").
- A three-card table: each app's icon, one line, latest release badge (`img.shields.io/github/v/release/Edgar-Mendonca/Datum-Plan`), and **Download** / **Repo** links.
- **Why Datum:** small and fast, private, built for real technical work, one design across apps, free forever.
- **Principles:** Rust engine in a pure crate with tests; SolidJS window; one release = one small numbered version shown in the in-app updater; docs and memory kept in every repo.
- **How the family is built:** link to `FAMILY.md` and `MEMORY.md`; checklist for starting a new app.
- **Contribute / feedback:** issues in each app repo; ideas for new apps here.
- Licence MIT.

## 4. The website

Same look as the apps: light theme, IBM Plex Sans/Mono, cards with 1 px `--line` borders and 8 px radius, a navy header band, accent colour per app page (`--blue` swapped to the app accent, exactly as FAMILY.md §3 does in the apps). Dotted whiteboard background (as the Network Diagram) on the hero. No framework UI kit; plain Astro components and one CSS file of tokens. No client JavaScript except a tiny theme toggle (light/dark/auto).

Pages:
- **Home:** hero (logo, positioning line, "Free · Open source · Windows"), three app cards (icon, name, one line, latest version and date, Download button to `releases/latest`, Learn more), a "Built the same way" strip (four principles), latest releases across the family (the 6 newest, merged and sorted by date).
- **App page** (`/plan`, `/plot`, `/atlas`): hero in the app accent with icon, tagline, "for" line, Download and GitHub buttons; highlights; screenshot gallery; **What's done** (the app's last 5 CHANGELOG sections, rendered from Markdown); **Roadmap** (stages with Next/Soon/Later/Goal pills, same as File › Roadmap in the app); links to issues and releases.
- **Family:** the design principles, colour accents, type, logo rules (rendered from FAMILY.md sections).
- **About:** who builds it, how to contribute, licence.

## 5. Data: read from the app repos at build time

Each app repo publishes **`docs/site.json`** (Datum Plan already does; File › Roadmap in the app reads the same file):

```json
{
  "id": "plan", "name": "Datum Plan", "accent": "#0B5CAD",
  "logo": "docs/brand/datum-plan-icon.svg",
  "tagline": "…", "for": "…",
  "highlights": ["…"],
  "screenshots": ["docs/images/gantt.png"],
  "roadmap": [{ "v": "0.3.10", "title": "Fixes", "when": "Next", "items": ["…"] }]
}
```

`apps.json` in DATUM lists the repos (`[{ "id": "plan", "repo": "Edgar-Mendonca/Datum-Plan", "branch": "<default>" }, …]`); adding an app = one line.

`src/lib/github.ts` (build time only, Node `fetch`, `GITHUB_TOKEN` from Actions to avoid rate limits):
- `site.json`, `CHANGELOG.md`, logo SVG from `raw.githubusercontent.com/<repo>/<branch>/…`
- latest release from `api.github.com/repos/<repo>/releases/latest` (tag, date, installer asset URL and size)
- screenshots linked from raw URLs (or copied into `dist/` at build to avoid hotlinking)
- **Fallbacks:** if a file or release is missing (a new app), show "Coming soon" instead of failing the build. Never fail the build on one app.

CHANGELOG parsing: split on `## X.Y.Z — date`, render bullets with Astro's Markdown (`marked` is fine).

## 6. Deploy

`.github/workflows/pages.yml`:
- on: `push` to main, `schedule` (daily, e.g. `17 5 * * *`), `workflow_dispatch`, and `repository_dispatch` (type `app-released`).
- jobs: checkout → setup Node 20 → `npm ci && npm run build` in `site/` with `GITHUB_TOKEN` → `actions/upload-pages-artifact` (`site/dist`) → `actions/deploy-pages`.
- permissions: `contents: read`, `pages: write`, `id-token: write`.
- Settings › Pages › Source: **GitHub Actions**.

Optional later: each app's `release.yml` sends `repository_dispatch` to DATUM after publishing (needs a fine-grained token secret), so the site updates within minutes of a release. The daily build covers it until then.

## 7. Quality bar

- Lighthouse 95+ on all four scores; total page weight under 300 KB without screenshots; fonts self-hosted or from Google Fonts with `display=swap`.
- Works at 360 px wide; images have alt text; colour contrast AA.
- `npm run build` passes locally with no token (unauthenticated fetch) for a quick check.

## 8. First session in the DATUM repo (checklist)

1. Add `Edgar-Mendonca/Datum-Plan`, `Datum-Plot`, `Datum-Atlas` read-only; read this brief and `Datum-Plan/docs/FAMILY.md`.
2. Write `README.md`, `FAMILY.md`, `MEMORY.md`, `apps.json`, `brand/`.
3. Build the Astro site with the data layer and fallbacks; check it locally with Playwright screenshots at 1280 and 375 px.
4. Add `pages.yml`, push, ask the user to set Pages › Source to GitHub Actions, confirm the live URL.
5. List which app repos still need a `docs/site.json` (Plot, Atlas) and give the user the snippet to add in those chats.
