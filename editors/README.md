# Lini editor grammars

Syntax highlighting for `.lini` files. Both editors' **keyword lists — types,
templates, properties, value builders, and layout names — are generated from the
same property ledger the compiler reads** (`cargo xtask gen-grammars`), so a new
type or property highlights the moment it has a row. `tests/grammar.rs` regenerates
both files in memory and asserts byte-equality with what is committed here, exactly
as the schema is guarded — a stale checkout fails CI. Never hand-edit the generated
files; edit `src/grammar/mod.rs` and regenerate.

## VS Code (`vscode/`)

A TextMate bundle — `syntaxes/lini.tmLanguage.json` (generated),
`language-configuration.json`, and `package.json`. To try it locally:

```bash
cp -r editors/vscode ~/.vscode/extensions/lini
# reload VS Code; open any .lini file
```

Highlights comments, strings, numbers, `|type#id|` identity bars, `.class`, `#id`,
`--var`, the link operators (`->`, `<->`, `--*`, `~>`, `&`, …), `key:` property
names (strong scope for ledger rows, weak for unknowns), value builders
(`gradient(`, `oklch(`, …), enum/value keywords, and the `( )` math expressions.

## Zed (`zed/`)

A tree-sitter extension — `extension.toml`, `languages/lini/config.toml`, the
generated `languages/lini/highlights.scm`, and the grammar source under
`tree-sitter-lini/`. The highlight query classifies nodes through `#match?`
predicates carrying the ledger keyword sets (the generated, drift-guarded part).

**Packaging status:** the parser is not built or smoke-tested in-repo — that needs
the tree-sitter CLI and Zed, which run at release time:

```bash
cd editors/zed/tree-sitter-lini && tree-sitter generate && tree-sitter test
```

Then set `[grammars.lini].commit` in `extension.toml` to the published tag and
install the dev extension from Zed's extensions panel.
