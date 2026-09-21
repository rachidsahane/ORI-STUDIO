# apps/desktop/ui

The webview half of the desktop application. SolidJS, Vite, TypeScript with
`strict: true` (`spec/LLD.md` section 3; `spec/CONVENTIONS.md`, "TypeScript and
UI"). Nothing here is built in phase 1; this tree is the shape the phase 3
tickets fill.

## Layout

`spec/LLD.md` section 3 names the structure: by screen, with shared
`components/`, an `rpc/` client generated from API_SPEC, and `store/` holding
Solid stores fed by events.

| Path | What it is | State in this tree |
|---|---|---|
| `ui/src/screens/` | One directory per screen, the twelve `spec/LLD.md` section 3 names | Present, README only |
| `ui/src/components/` | Shared presentational components | Present, README only |
| `ui/src/rpc/` | The typed client generated from `spec/API_SPEC.md` | Present, README only |
| `ui/src/store/` | Solid stores, fed by engine events | Present, README only |
| `ui/src/tokens.css` | Design tokens; `spec/design/DESIGN.md` section 2 names this path as their source of truth | Absent, see `../README.md` |

The third column is there so that no reader has to infer the state of this tree
from a layout diagram. Four of the five names exist; `tokens.css` does not, and
the row says so. Every name this tree omits is listed with its reason in
`../README.md`, "What deliberately does not exist, and why"; a name missing from
this tree and absent from that table is a defect, not a deferral.

**Why `src/`.** `spec/LLD.md` section 3 writes the names without a prefix
(`screens/dashboard`, `components/`, `rpc/`, `store/`) and does not say where
they sit inside `ui/`. `spec/design/DESIGN.md` section 2 names one concrete path
inside this directory, `ui/src/tokens.css`, and `spec/LLD.md` section 3 says the
UI is built with Vite, whose project root holds the tooling files and whose
sources sit in `src/`. Placing LLD's four names under `src/` satisfies both
documents. This is a reading, not a specification change; if the lead prefers
them directly under `ui/`, moving them is a rename and touches no other document.

## Rules that apply to everything under this directory

From `spec/CONVENTIONS.md` and `spec/LLD.md` section 3:

- `strict: true`. No `any`.
- Components small and pure; state lives in Solid stores fed by events, not in
  component-local state that another component has to guess at.
- **No direct engine access other than the generated RPC client in `rpc/`.** A
  `fetch`, a Tauri `invoke` or an event listener anywhere but `rpc/` and
  `store/` is the private path into the engine that `spec/ARCHITECTURE.md`
  section 4 forbids.
- Accessibility: keyboard navigation for every action, ARIA roles on cards and
  queues, and color is never the only signal. `spec/design/DESIGN.md` section 2
  repeats the last rule: every state carries a glyph or a label as well as a
  color.
- Monaco is loaded lazily and is read-only everywhere except documents under
  `spec/` (`spec/LLD.md` section 3).
- xterm.js is connected to the `terminal.*` RPC, and input is sent through RPC
  so that it is logged (`spec/LLD.md` section 3). Terminal input that bypasses
  RPC is an unattributed change waiting to happen; see PRD Z-01.
- What the surface must not show is as binding as what it shows. AICD §28 names
  three things: agent working memory and intermediate reasoning, raw production
  telemetry inside the review interface, and vanity metrics (lines produced,
  commits, tickets closed without their category and tier). A screen that adds
  any of them is a specification question, not a UI ticket.

## Risk tier

`spec/RISK_MAP.md`: `apps/desktop/ui` is "0 for styling and layout, 1 for
anything that sends an RPC". In practice everything under `rpc/` and `store/`,
and any screen component that calls them, is tier 1.

## Screens named elsewhere that this tree does not yet carry

`src/screens/` follows `spec/LLD.md` section 3, because LLD is the authority on
layout. It is not the authoritative screen set: `spec/design/DESIGN.md` governs
that for phase 3.

The three documents diverge. `spec/LLD.md` section 3 names twelve screens.
`spec/PRD.md` section 6 names fifteen: the twelve plus Home (projects,
portfolio), Calibration and cost, and Audit trail. `spec/design/DESIGN.md`
section 1 describes the reference as covering eighteen. Reconciling them is a
specification question for the documentation role and the lead is carrying it to
a specification pull request, so this ticket created exactly the twelve
directories LLD section 3 names, added none of its own and removed none. See
`src/screens/README.md`.

## Build

There is none yet, on purpose. `spec/ENV_SETUP.md` section 1 names Node LTS and
pnpm for the UI build only, "never a runtime dependency of the engine". The
phase 3 ticket that introduces the build adds `package.json`, `vite.config.ts`,
`tsconfig.json` and `index.html` at `ui/`, and the dependency review that comes
with them.
