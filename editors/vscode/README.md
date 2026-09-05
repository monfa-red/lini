# Lini for VS Code

Syntax highlighting for [Lini](https://github.com/monfa-red/lini) (`.lini`) — a
text-to-diagram language for diagrams, charts, engineering drawings, floor
plans, and circuit schematics that compiles to clean, themeable SVG.

```lini
cat -> dog -> bird
```

Highlights comments, strings, numbers, `|type#id|` identity bars, `.class`,
`#id`, `--var` references and `--var:` declarations, `name = value` bindings,
the link and measuring operators (`->`, `<->`, `~>`, `(-)`, `(o)`, `(<)`,
`>-`, `||`), property names (generated from the compiler's own property
ledger, so they never drift), value builders (`gradient(`, `oklch(`,
`repeat(`, …), and `( )` math expressions.

Install the `lini` CLI to compile: `cargo install lini` — see the
[repository](https://github.com/monfa-red/lini) for the language guide, the
playground (`lini serve`), and the samples.
