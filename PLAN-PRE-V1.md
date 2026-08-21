# PLAN-PRE-V1 — battle-hardening before `1.0.0`

The release-readiness rounds between the beta tags and `1.0.0`. (Supersedes
`plans/PLAN-V1.md`, whose feature rounds all shipped — its live remainder, the
rc ladder, is chunk 9 here.) One lens drives
every chunk here: **anything accidentally lenient today becomes frozen API;
anything that errors today stays a free option.** So the work is: make every
promised gate actually fire, pin every intentionally-open surface as an error,
freeze the output/tooling API deliberately, settle the default look once, and
then battle-test the whole thing with fresh eyes before tagging.

**How to run.** Chunks 1–5 are agent-sized: dispatch **one Opus agent at a
time, `xhigh` effort, model set explicitly** — never inherited. Every brief
starts with: *read `SPEC.md` and `ROUTING.md` fully, then `AGENTS.md`*; and
carries the duplication check verbatim: *when two places do the same job they
call one shared function — extend whatever owns a failure mode, never add a
parallel copy.* After each chunk: `cargo fmt`, `cargo test`, `cargo clippy`,
commit, push. Chunk 6 is interactive (taste), done with the user. Chunks 7–8
are their own fresh sessions.

---

## Chunk 1 — close the leniency gates

The SPEC promises errors the code doesn't raise (the 2026-08 audit's item 6).
Each is a de-facto relaxation shipping by accident.

- **Out-of-scope gates for chart types**: `|bars|` / `|dots|` / `|area|` /
  `|axis|` / `|band|` / `|mark|` / `|bubble|` in a non-chart scope compile
  clean today; `|slice|` outside a pie is only half-gated. Tree, sequence,
  schematic, and drafting types all have the equivalent gate — extend that one
  mechanism, don't write a chart-only checker.
- **Colour validation**: `fill: xyzzy` and `rgb(300, 0, 0)` are accepted
  silently; SPEC 2 promises `invalid color` / component-out-of-range errors.
- **Five SPEC 21 rows fall through to generic messages**: text-with-children,
  spaced call paren, minted-ref endpoint, dot-path into a label, and the
  `pins:` / `number:` value shapes. Each gets its targeted message.
- The define-cycle message prints ASCII `->` in `types.rs` but `→` in
  `theme.rs` — unify on `→`.

**Acceptance**: every touched SPEC 21 row has a test asserting its exact
message; no new mechanism — each gate extends the existing owner of that
failure mode.

## Chunk 2 — the reserved-surface audit

Enumerate every intentionally-unbuilt surface and pin today's behaviour as an
**error test**, so no accidental leniency ships and every deferred feature
stays a free option. Sources: SPEC 24 (all sections), plus these findings:

- `routing:` in a **link's own block** (per-link routing is not a thing — one
  scope, one strategy) — must error, not silently ignore.
- A one-ended link op before a string or capsule **outside** schematic /
  drawing scopes (the future flow-callout slot).
- A capsule endpoint in a drawing (gated — keep the test pinning it).
- `%` outside colour components; `fr`-like or fractional track values in
  `columns:` (the future equal-track slot); `legend:` position values;
  per-axis tick text alongside `categories:`.
- Anything else SPEC 24 names whose surface is *parseable* today — the agent
  sweeps SPEC 24 exhaustively and adds one test per reachable slot.

**Acceptance**: one test file (or module) that reads as the deferred-surface
ledger — a reviewer can diff SPEC 24 against it. Behaviour changes belong in
chunk 1; this chunk only *pins* (a slot found lenient here moves to chunk 1's
pattern and gets gated).

## Chunk 3 — SPEC fenced-block CI

Compile every fenced code block in `SPEC.md` (and `ROUTING.md` if any) in CI —
a `fragment` marker exempts the ~15 context-free snippets. This is the
highest-leverage doc guard: it has already caught four broken examples once,
by hand. Wire it as a test (or `xtask`) that runs in the normal suite.

**Acceptance**: `cargo test` fails when a SPEC example stops compiling; the
fragment allowlist is explicit in the test, not scattered in the doc.

## Chunk 4 — freeze the output & tooling API

The compat surface beyond syntax, audited once, deliberately:

- **Inventory the emitted `lini-*` classes and `data-*` attributes** against
  SPEC 18's hook-family table; fix drift in whichever side is wrong; add a
  test that renders a kitchen-sink scene and asserts the emitted class set ⊆
  the documented set.
- **Pin the `--json` diagnostics schema** (codes, severity, spans, fixes) with
  a snapshot test; note stability in SPEC 21 (codes stable, messages may
  improve — already promised; now enforced).
- **State the bidi/RTL position** in SPEC 24: deferred; text renders in LTR
  glyph order today. One honest paragraph, no code.
- Fix the two known drawing chrome nits while in the render layer: the detail
  boundary ring draws width 2 where the rule says 1; `.lini-dim-line`'s
  `stroke-width: 1` is hardcoded — seed it from the drawing link defaults like
  the tone, so `|-| { stroke-width: 3 }` works.

**Acceptance**: class-set test + `--json` snapshot green; SPEC 18/21/24
amended; the two chrome nits verified by rendered PNG.

## Chunk 5 — grammar single-source

The grammar lives in three homes (lexer / generated editor grammars /
playground tokenizer). Build `xtask gen-grammars` so the editor grammars and
tokenizer generate from one source, with a CI check that they're current.
(Deferred from the 2026-08 audit; do it now so post-1.0 syntax additions can't
drift the homes apart.) If a ledger value-set column falls out naturally —
feeding grammars / schema / validate / suggest — take it; don't force it.

**Acceptance**: editing the grammar in one place regenerates the others;
CI fails on a stale copy.

## Chunk 6 — the defaults taste pass (interactive)

Defaults are the frozen look of 1.0. Known direction from the user: **`gap` is
too tight** (root/flow 20 → try 24 / 28 / 32), **`clearance` maybe larger**
(16 → try 20 / 24). Also on the table for one look: root `padding` 20,
`font-size` 15, tree `gap: 64 48`, chart 360×220.

Method: render a matrix of representative samples (hero, links_hard, tree,
mindmap, sequence, charts) at the candidate values to PNGs; the user picks;
land the winners in the ledger (one home per default); re-snapshot everything
**once**, after chunks 1–5, so snapshot churn happens a single time. SPEC 10.5
/ ledger prose updated to match.

## Chunk 7 — the battle-test gauntlet (fresh session)

The see-into-the-future pass: simulate real users before real users arrive.
Protocol, one persona per run, **one agent at a time**:

- Personas: README flowchart · org chart + mindmap · KPI dashboard · ER
  diagram · sequence doc · drafted part · PSU schematic · **website embedding
  + theming lini SVGs** (the case that already surfaced real issues).
- Each persona gets a realistic deliverable and **does not read the SPEC
  first** — it works from the README/quickstart the way a new user would,
  writes real `.lini`, compiles with the real binary, renders PNG, looks at
  it, and logs every wall: wrong first guess, confusing error, ugly default,
  missing feature, docs gap.
- Output per persona: the `.lini` files + a findings log. Then triage
  together: fix now / relax later (verify it errors) / defer / docs.
- Good files feed the samples redo (its own backlog item).

## Chunk 8 — SKILL.md (fresh session)

An agent-facing skill so new users' agents can write lini well: the mental
model in agent-optimal order, the sharp edges (comma law, tail ownership,
scope sealing, layout-owned properties), a worked example per layout, and the
compile-render-look loop. Needs its own session to perfect and token-optimize;
battle-test findings (chunk 7) feed it — write it after.

## Chunk 9 — rc → 1.0.0 (carried from PLAN-V1)

- Bug fixes only past the rc tag; anything feature-shaped goes to ROADMAP 6.
- The stability contract (ROADMAP section 2) lands **in SPEC** as a normative
  section — and states the compat policy this plan enforces: syntax and
  diagnostic codes stable; rendered output may improve between minors (no
  per-file version marker).
- Full visual review: every sample, light + dark, screen + print scale.
- Cut `1.0.0` when an rc survives with zero code changes needed.

---

## Not in this plan

- **Samples redo/consolidation** — its own session (existing backlog item);
  runs best after chunks 6–7 so it lands on final defaults and real findings.
- **wasm lazy icon blob + npm publish** — deferred (backlog items 8–9).
- **Deferred features** (`fr` tracks, inline rich-text spans, per-link
  routing, flow callouts, balloon-capsule leaders) — consciously post-1.0;
  chunk 2 pins each as an error so they stay free options.
