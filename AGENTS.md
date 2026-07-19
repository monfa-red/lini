# Agent guide

How to work in this repo.

## Communication
- **Short by default: aim for ≤5 sentences.** Go longer only when the
  substance earns it — a real finding, a tradeoff the user must weigh —
  and even then, tight. Code blocks and tool output don't count.
- Say each thing once. No preamble, no restating the question, no recap
  of what was already said, no closing summary.
- No headers or bold walls; bullets/tables only when the answer is
  genuinely a list or a comparison.
- Explain only when asked, and keep it tight. Short beats well-worded.
- Don't narrate intentions; do the thing and report what changed.
- Exploratory questions: propose one path + the main tradeoff, then wait — don't decide and implement.
- Ask before risky or irreversible actions (force push, destructive ops, publishing). Local edits are free.

## Subagents
- **Always set `model` explicitly when spawning agents** (Agent tool,
  workflow `agent()`): the main session may run a pricier model and a
  subagent must never inherit it. Forks are the one exception.
- Pick the tier by the task: **`opus` at `xhigh` effort** for most real
  work (investigation, implementation, review); **`opus` at `high`** for
  simpler scoped tasks; **`sonnet` at `high`** for genuinely simple
  mechanical ones (lookups, renders, bulk edits).
- Per-call `effort` exists only on workflow `agent()`; the Agent tool
  takes `model` alone — there, choose the tier by model and let effort
  ride the agent definition.

## Code style
- **No `unsafe`.** Find another path, or surface the question.
- **One mechanism per problem.** Extend whatever owns a failure mode; never add a second that re-fights it. Robust fixes over patches.
- **No parallel implementations.** When two places do the same job — a node's text leaf and a link's label, the node and link sides of a sugar rule — they call **one** shared function; never copy logic (whole or in part) between them. Divergent copies drift: a fix lands in one path and the other silently rots (translate worked on one text, broke on the other). Factor the shared part out and reuse it; keep only the genuinely-different slice per caller.
- **Modular: one concept per file.** Split a module past ~500 LOC.
- Standard idioms over clever code; don't fight `rustfmt` / `clippy`.
- **Reused style rides a rule, never inline.** Anything stated once and
  worn many times — a generated class (hue walk, depth ramp), engine
  chrome, a template look — emits **one CSS rule** and a class on each
  wearer; `style=` is only ever the *diff* an element authors against its
  rules ([SPEC 17]'s class-diff). A dead class on an element whose look
  was inlined is the smell: the rule exists, emit it.
- **Trust a correct model.** Don't special-case a principled formula's output to nudge one case to taste — fix the model, or accept the result.
- Nothing beyond the task — no extra features, validation, or comments (comments only for the non-obvious *why*).
- Cosmetics last: pure-looks polish goes in a final pass.

## Testing
- `insta` snapshot tests for any output-shaped code.
- Samples are the showroom; one sample per feature **cluster** — extend an
  existing sample before adding a file.
- Verify SVG visually — render to PNG with `resvg` and read it; don't make the user spot-check.

## Git
- Descriptive messages (what changed and why); one purposeful change per commit.
- **Never include "Co-Authored-By" lines.**
- **Before every push run `cargo fmt`** — CI runs `cargo fmt --all -- --check` and fails on any diff (also run `cargo test` and `cargo clippy`).
- Defer pushing to `main` to the user.

## Re-orient (fresh session)
Read `SPEC.md` (the language) and `ROUTING.md` (the routing contract), then skim `git log` and run `cargo test`. Active plans live in the repo root; completed round docs archive to `plans/` (never a `docs/` folder).
