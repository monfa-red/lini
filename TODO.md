# TODO

## Better diagnostics / error reporting

Today many misuses are **silently ignored** rather than reported — the value
just does nothing, so the author has no idea why. Build a context-aware
diagnostic pass that says *what* is wrong and *where*, with a hint.

Cases to cover (grow this list):

- **Property used where it can't apply.** A wire is not a box, so `translate`,
  `pin`, `padding`, `width`, etc. on a wire (or directly on a bare wire label)
  do nothing — error instead. Example:
  `a -> b { translate: 0 -8; "x" }` → *`'translate' is not valid on a wire — put
  it on a `|plain|` label: `{ |plain| { translate: 0 -8; "x" } }``*
- **Grid props off a grid** already error (`cell`/`span`/`columns`); fold into
  the same pass.
- **Unknown property name**, with a "did you mean" hint table (SPEC §19 lists
  this as deferred) — e.g. `paddding:` → *did you mean `padding`?*
- **Value out of range / wrong shape** — e.g. `translate: 0 -10 0` (3 values),
  `pin: middle` (not an anchor) — point at the offending token.

Design notes:
- One pass, keyed by node kind (box / text / wire / wire-label) → the set of
  properties valid on it; anything else warns (or errors under `--strict`).
- Keep messages LSP-formatted (`file:line:col: error: …`) like the rest.
