# ui/src/components

Shared presentational components: the ones more than one screen uses.
`spec/LLD.md` section 3 names this directory "shared `components/`".

## What belongs here

A component used by two or more screens. Small and pure
(`spec/CONVENTIONS.md`, "TypeScript and UI"): props in, markup out, no store
access of its own and no RPC call. A component that needs data takes it as a
prop; the screen reads it from `../store`.

The recurring vocabulary of `spec/design/DESIGN.md` lives here: tier chips,
category chips, state chips, cards, queues, the coverage matrix, the diff view.

## What does not

- A component only one screen uses. It stays in that screen's directory until a
  second screen needs it.
- Anything that calls `../rpc` or reads `../store` directly. That is the
  screen's job, and it is what keeps `spec/RISK_MAP.md`'s "0 for styling and
  layout" true of this directory.

## Rules

- Keyboard navigation for every action; ARIA roles on cards and queues
  (`spec/CONVENTIONS.md`).
- Color is never the only signal: every state carries a glyph or a label as well
  (`spec/CONVENTIONS.md`; `spec/design/DESIGN.md` section 2).
- Colors come from the tokens in `../tokens.css`, never as literals, so that the
  light theme stays a token file rather than a redesign
  (`spec/design/DESIGN.md` section 2). That file is not in this tree yet and
  `../../README.md` records why; until the phase 3 ticket authors it,
  `spec/design/DESIGN.md` section 2 holds the token names and values.
