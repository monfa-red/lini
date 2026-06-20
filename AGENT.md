# Agent guide

How to work in this repo.

## Communication
- **Keep replies short.** No preamble, recap, or closing summary — a sentence or a small table beats a paragraph.
- Don't narrate intentions; do the thing and report what changed.
- Exploratory questions: propose one path + the main tradeoff, then wait — don't decide and implement.
- Ask before risky or irreversible actions (force push, destructive ops, publishing). Local edits are free.
- No emojis unless asked. Plain Markdown; tables over bullets when comparing.

## Code style
- **No `unsafe`.** Find another path, or surface the question.
- **One mechanism per problem.** Extend whatever owns a failure mode; never add a second that re-fights it. Robust fixes over patches.
- **Modular: one concept per file.** Split a module past ~500 LOC.
- Standard idioms over clever code; don't fight `rustfmt` / `clippy`.
- Nothing beyond the task — no extra features, validation, or comments (comments only for the non-obvious *why*).
- Cosmetics last: pure-looks polish goes in a final pass.

## Testing
- `insta` snapshot tests for any output-shaped code.
- One sample per feature in `samples/`.
- Verify SVG visually — render to PNG with `resvg` and read it; don't make the user spot-check.

## Git
- Descriptive messages (what changed and why); one purposeful change per commit.
- **Never include "Co-Authored-By" lines.**
- **Before every push run `cargo fmt`** — CI runs `cargo fmt --all -- --check` and fails on any diff (also run `cargo test` and `cargo clippy`).
- Defer pushing to `main` to the user.

## Re-orient (fresh session)
Read `SPEC.md` (the language) and `WIRING.md` (the routing contract), then skim `git log` and run `cargo test`.
