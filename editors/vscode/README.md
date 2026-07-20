# Lini for VS Code

Syntax highlighting for [Lini](https://github.com/monfa-red/lini) (`.lini`) — a
text-to-diagram language for diagrams, charts, and technical drawings that
compiles to clean, themeable SVG.

```lini
cat -> dog -> bird
```

Highlights comments, strings, numbers, `|type#id|` identity bars, `.class`,
`#id`, `--var`, the link and measuring operators (`->`, `<->`, `(-)`, `(o)`,
`(<)`, `>-`, `||`), property names (generated from the compiler's own property
ledger, so they never drift), value builders, and `( )` expressions.

Install the `lini` CLI to compile: `cargo install lini` — see the
[repository](https://github.com/monfa-red/lini) for the language guide.
