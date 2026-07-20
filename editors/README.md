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

**Packaging status:** the parser **is** built and sample-swept in-repo — the
generated parser (`tree-sitter-lini/src/`, ABI 14) is committed and
`[grammars.lini].commit` in `extension.toml` is pinned to the commit that carries
it, so Zed can fetch a working grammar. What remains is only the in-Zed
dev-extension smoke test: install it from Zed's extensions panel (Install Dev
Extension → point at `editors/zed`) and eyeball a `.lini` file.

Regenerate and re-run the zero-ERROR sweep at release time (node at
`/opt/homebrew/opt/node@24/bin`; the CLI is fetched on demand):

```bash
cd editors/zed/tree-sitter-lini
npx --yes tree-sitter-cli@latest generate --abi 14        # rebuilds src/
# every sample must parse with zero ERROR nodes:
for f in ../../../samples/*.lini; do \
  echo "$(npx --yes tree-sitter-cli@latest parse "$f" 2>&1 | grep -c ERROR)  $f"; \
done
```

After regenerating, run `cargo test` (the drift test guards `highlights.scm`), then
if the grammar source changed commit `src/` and re-pin `[grammars.lini].commit`.
