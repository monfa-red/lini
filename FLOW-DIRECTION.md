# `layout: flow` + `direction` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split layout into an *engine* (`layout: flow | grid | chart | pie`) and an *orientation* (`direction: row | column`), dropping `layout: row` / `layout: column`.

**Architecture:** `layout` already selected the box-arranger mode; `direction` already existed as a chart-only orientation. This unifies them: `layout` picks the engine, `direction` orients a `flow` (and still a chart). `LayoutMode` collapses from `{ Row, Column, Grid }` to `{ Flow, Grid }`; the flow axis comes from a new `read_flow_direction` reading `direction`. Chart/pie are a separate engine intercepted before `read_layout_mode`, so they're untouched. The change was already foreseen — CHARTS.md §11 calls it "a core `layout`/`direction` split … planned for SPEC.md."

**Tech Stack:** Rust (no `unsafe`), `insta` snapshot tests, `resvg` for visual verification.

## Global Constraints

- **No backward compatibility.** `layout: row` / `layout: column` are removed and become hard errors *as if they never existed* — with a hint pointing to `direction:`.
- `direction` for **flow** accepts `row` · `column` (default `column`). `direction` for **charts** is unchanged (`row` · `column` · `radial`, default `column`).
- Idiom for migrated source: `layout: row` → `direction: row`; **drop** redundant `layout: flow`, `layout: column`, and `direction: column` (they are defaults). While migrating samples, also drop *any* re-applied default (a property whose value equals that node's built-in default).
- Keep `|row|` / `|column|` sugar (redefined); **add `|grid|`** sugar for `layout: grid`.
- No `unsafe`. No parallel implementations. Run `cargo fmt` before any push; CI checks `cargo fmt --all -- --check`, `cargo test`, `cargo clippy`.
- Verify SVG visually: render to PNG with `resvg` and read it.

---

### Task 1: Core layout/direction split (`src/layout/mod.rs`)

**Files:**
- Modify: `src/layout/mod.rs` — `LayoutMode` (549-556), `read_layout_mode` (558-572), new `read_flow_direction`, dispatch (421-463), divider gate (491-501), `one_d_dividers` (351-378), the in-file comment (263), and the two flex tests (687, 704) + a new error test.

**Interfaces:**
- Consumes: `flex::Axis` (already imported at `mod.rs:22` as `use flex::Axis;`), `Error::at`, `ResolvedValue::Ident`.
- Produces: `enum LayoutMode { Flow, Grid }`; `fn read_layout_mode(&AttrMap, Span) -> Result<LayoutMode, Error>`; `fn read_flow_direction(&AttrMap, Span) -> Result<Axis, Error>`; `fn one_d_dividers(&[PlacedNode], &[usize], Axis, Bbox) -> Vec<GridRule>`.

- [ ] **Step 1: Write the failing tests** — add to `mod.rs` `tests` module, and convert the two existing flex tests.

Convert `row_layout_stacks_horizontally` (687) source `"{ layout: row; gap: 10; }\n…"` → `"{ direction: row; gap: 10; }\n…"`.
Convert `column_layout_stacks_vertically` (704) source `"{ layout: column; gap: 20; }\n…"` → `"{ direction: column; gap: 20; }\n…"`.
Add:

```rust
    #[test]
    fn layout_row_and_column_are_removed() {
        for bad in ["row", "column"] {
            let src = format!("{{ layout: {bad}; }}\n|box|\n|box|\n");
            let tokens = crate::lexer::lex(&src).expect("lex");
            let file = crate::syntax::parser::parse(&tokens).expect("parse");
            let lowered = crate::desugar::desugar(&file).expect("desugar");
            let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
            let err = layout(&program).expect_err("layout: row/column must error");
            assert!(
                err.message.contains(&format!("direction: {bad}")),
                "msg={}",
                err.message
            );
        }
    }

    #[test]
    fn direction_radial_is_rejected_in_a_flow() {
        let tokens = crate::lexer::lex("{ direction: radial; }\n|box|\n|box|\n").expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let err = layout(&program).expect_err("radial flow must error");
        assert!(err.message.contains("chart"), "msg={}", err.message);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib layout:: 2>&1 | tail -20`
Expected: the two new tests FAIL to compile/assert (`layout: row` currently succeeds, returning `LayoutMode::Row`).

- [ ] **Step 3: Replace the enum + readers** — `mod.rs:549-572`:

```rust
/// Container layout engine, parsed from the `layout=` attr. Chart/pie are a
/// separate engine intercepted in `layout_inst` *before* this runs, so this only
/// ever sees the box-arranger's two modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutMode {
    /// 1-D flex; its axis comes from `direction` (`read_flow_direction`).
    Flow,
    /// 2-D grid; sized by its `columns` / `rows` track lists (read in `grid`).
    Grid,
}

fn read_layout_mode(attrs: &crate::resolve::AttrMap, span: Span) -> Result<LayoutMode, Error> {
    match attrs.get("layout") {
        None => Ok(LayoutMode::Flow),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "flow" => Ok(LayoutMode::Flow),
            "grid" => Ok(LayoutMode::Grid),
            // Removed: orientation moved to `direction` (SPEC §5).
            dir @ ("row" | "column") => Err(Error::at(
                span,
                format!(
                    "'layout: {dir}' is not a layout — flow is the default; set 'direction: {dir}'"
                ),
            )),
            other => Err(Error::at(
                span,
                format!("unknown layout '{other}' — expected flow or grid"),
            )),
        },
        Some(_) => Err(Error::at(span, "'layout' expects flow or grid")),
    }
}

/// A flow's main axis from `direction` (SPEC §5), default `column`. `radial`
/// belongs to a chart, which owns its subtree and never reaches here.
fn read_flow_direction(attrs: &crate::resolve::AttrMap, span: Span) -> Result<Axis, Error> {
    match attrs.get("direction") {
        None => Ok(Axis::Column),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "column" => Ok(Axis::Column),
            "row" => Ok(Axis::Row),
            "radial" => Err(Error::at(
                span,
                "'direction: radial' is only valid in a chart — a flow is row or column",
            )),
            other => Err(Error::at(
                span,
                format!("unknown direction '{other}' — expected row or column"),
            )),
        },
        Some(_) => Err(Error::at(span, "'direction' expects row or column")),
    }
}
```

- [ ] **Step 4: Rewire the dispatch** — `mod.rs:422` (after `let mode = read_layout_mode(...)?;`) add the axis read, and replace the `match mode` body (439-463) Flow/Grid arms:

```rust
    let mode = read_layout_mode(container_attrs, span)?;
    // A flow's axis comes from `direction`; a grid has none.
    let flow_axis = match mode {
        LayoutMode::Flow => Some(read_flow_direction(container_attrs, span)?),
        LayoutMode::Grid => None,
    };
```

Replace the `let bbox = match mode { … }` arms:

```rust
        let bbox = match mode {
            LayoutMode::Flow => flex::lay_out_flex(
                flow_axis.expect("a flow has an axis"),
                &mut flow_children,
                container_attrs,
                span,
                avail,
            )?,
            LayoutMode::Grid => {
                // A table (a grid with dividers) reads `padding` as the per-cell
                // inset (SPEC §8): inflate each cell so auto tracks size to
                // content + inset and the text centres with that breathing room.
                if grid::is_inset_grid(container_attrs) {
                    for c in &mut flow_children {
                        c.bbox = c.bbox.expand(pad.top, pad.right, pad.bottom, pad.left);
                    }
                }
                let (bbox, rules) = grid::lay_out_grid(&mut flow_children, container_attrs, span)?;
                grid_rules = rules;
                bbox
            }
        };
```

- [ ] **Step 5: Rewire the 1-D divider gate** — replace `mod.rs:491-501`:

```rust
    if let Some(axis) = flow_axis {
        if grid::read_divider(container_attrs) != grid::Divider::None && flow_indices.len() > 1 {
            grid_rules = one_d_dividers(
                children,
                &flow_indices,
                axis,
                flow_bbox.shifted(off_x, off_y),
            );
        }
    }
```

And change `one_d_dividers` (351-357) signature + first line:

```rust
fn one_d_dividers(
    children: &[PlacedNode],
    flow: &[usize],
    axis: Axis,
    flow_bbox: Bbox,
) -> Vec<GridRule> {
    let row = axis == Axis::Row;
```

- [ ] **Step 6: Fix the stale comment** — `mod.rs:263`: change "before the row/column/grid path (`read_layout_mode` rejects `layout: chart`)" to "before the flow/grid path (`read_layout_mode` only handles flow and grid)". Also `chart/mod.rs:43` comment "the same key `read_layout_mode` owns" stays accurate.

- [ ] **Step 7: Run tests**

Run: `cargo test --lib 2>&1 | tail -25`
Expected: PASS (incl. the new error tests and the two converted flex tests). `row=true/false` divider geometry unchanged.

- [ ] **Step 8: Commit**

```bash
git add src/layout/mod.rs
git commit -m "feat(layout): split layout into engine (flow/grid) + direction; drop layout:row/column"
```

---

### Task 2: Sugar templates + scene default

**Files:**
- Modify: `src/desugar/bundles.rs:141-142` (`|row|`/`|column|`), add `|grid|`; `root_defaults` (194).
- Modify: `src/desugar/types.rs:23-25` (add `("grid", "block")`).
- Modify: `src/desugar/bundles.rs` tests (263-269 area) — add a `|grid|`/`|row|` assertion.

**Interfaces:**
- Consumes: `id(name, value)` helper, `TEMPLATES` table, `is_template`.
- Produces: `|row|` → `[direction: row]`, `|column|` → `[direction: column]`, `|grid|` → `[layout: grid]`; root default `layout: flow`.

- [ ] **Step 1: Write the failing test** — in `bundles.rs` tests:

```rust
    #[test]
    fn flow_sugars_set_direction_and_grid_sets_layout() {
        assert_eq!(
            template_bundle("row").iter().find(|d| d.name == "direction")
                .and_then(|d| d.groups[0].first()),
            Some(&Value::Ident("row".into()))
        );
        assert_eq!(
            template_bundle("column").iter().find(|d| d.name == "direction")
                .and_then(|d| d.groups[0].first()),
            Some(&Value::Ident("column".into()))
        );
        assert_eq!(
            template_bundle("grid").iter().find(|d| d.name == "layout")
                .and_then(|d| d.groups[0].first()),
            Some(&Value::Ident("grid".into()))
        );
        assert!(!template_bundle("row").iter().any(|d| d.name == "layout"));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib bundles:: 2>&1 | tail -15`
Expected: FAIL — `|row|` currently sets `layout: row`, `|grid|` is unknown (`is_template("grid")` is false).

- [ ] **Step 3: Redefine the bundles** — `bundles.rs:141-142`:

```rust
        // Frameless flow wrappers over |block| (SPEC §8): the engine is flow by
        // default, so these only set the orientation. |grid| is the grid sibling.
        "row" => vec![id("direction", "row")],
        "column" => vec![id("direction", "column")],
        "grid" => vec![id("layout", "grid")],
```

- [ ] **Step 4: Register `|grid|` as a template** — `types.rs`, after the `("column", "block"),` line (24):

```rust
    ("grid", "block"),
```

- [ ] **Step 5: Flip the scene default** — `bundles.rs:194`:

```rust
        id("layout", "flow"),
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib bundles:: types:: 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/desugar/bundles.rs src/desugar/types.rs
git commit -m "feat(sugar): |row|/|column| set direction; add |grid|; root defaults to layout:flow"
```

---

### Task 3: Migrate existing tests off `layout: row/column`

**Files:**
- Modify: `tests/cli.rs:86,105`; `tests/linking.rs:338,342`; `tests/rendering.rs:722`; `src/fmt/tests.rs:133,134,142,143,150,151,252`.

**Interfaces:** none new — pure source-string edits. `direction: row` over the default `flow` is behaviorally identical to the old `layout: row`, so rendered SVG (and snapshots) stay byte-identical.

- [ ] **Step 1: Edit non-snapshot tests** — replace `layout: row` → `direction: row` in `tests/cli.rs:86` and `:105`; `tests/linking.rs:338` and `:342`; `tests/rendering.rs:722`.

- [ ] **Step 2: Edit formatter tests** — in `src/fmt/tests.rs`, replace `layout: column` → `direction: column` and `layout: row` → `direction: row` in both the input and the expected-output strings at 133/134, 142/143, 150/151, 252 (the structure/spacing under test is unchanged; only the property name differs).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | tail -30`
Expected: PASS, **no snapshot diffs** (`rendering.rs` SVG output is identical). If `insta` reports a pending snapshot, inspect it — it must be byte-identical; accept only if so.

- [ ] **Step 4: Commit**

```bash
git add tests/cli.rs tests/linking.rs tests/rendering.rs src/fmt/tests.rs
git commit -m "test: migrate fixtures to direction: row (flow is the default engine)"
```

---

### Task 4: SPEC.md

**Files:** Modify `SPEC.md` — §5 (441-535), §8 templates table (739-740) + `|block|` mentions (454, 1272), container-props table (528), inline examples (251, 963, 1773, 1784, 1787, 1795, 1798), layout-algorithm prose (1685-1688).

- [ ] **Step 1: Rewrite the §5 mode table** — replace lines 445-449:

```markdown
| Value | Behavior |
|---|---|
| `layout: flow` | 1-D flex. Orientation set by `direction` (default `column`). |
| `layout: grid` | 2D grid — sized by `columns` / `rows`. |

`direction: row | column` orients a flow (`column` stacks vertically — the default; `row` runs horizontally). `chart` / `pie` are separate engines, set via their templates ([§8](#8-templates)); a chart's `direction` also takes `radial` ([CHARTS.md](CHARTS.md)).
```

- [ ] **Step 2: Fix the defaults paragraph** — replace line 451 ("defaults to `layout: column`") with "defaults to `layout: flow` (a vertical `column`)". Keep the rest of the paragraph (454 `|block|`/`|row|`/`|column|` pad-by-0 sentence) unchanged.

- [ ] **Step 3: Container-props table** — replace the `layout` row (528) and add a `direction` row:

```markdown
| `layout` | all | `flow`, `grid` (chart / pie via templates). |
| `direction` | flow | `row` / `column`; orients a flow. Default `column`. (A chart's `direction` also takes `radial`.) |
```

- [ ] **Step 4: §8 templates table** — replace lines 739-740 and add `|grid|`:

```markdown
| `\|row\|` | `\|block\|` | `direction: row` | Frameless wrapper — children in a row. |
| `\|column\|` | `\|block\|` | `direction: column` | Frameless wrapper — children in a column. |
| `\|grid\|` | `\|block\|` | `layout: grid` | Frameless grid (needs `columns`). |
```

- [ ] **Step 5: Migrate inline examples** — in `SPEC.md`, apply the idiom: `layout: row` → `direction: row`; **delete** redundant `layout: column;` (and any trailing double-space). Lines: 251, 963, 1773, 1784, 1787, 1795, 1798. After deleting a bare `layout: column;`, leave the remaining decls intact (e.g. `cell: 2 1;  layout: column;  gap: 20;` → `cell: 2 1;  gap: 20;`).

- [ ] **Step 6: Layout-algorithm prose** — line 1687 "arrange flow children per `layout`" → "arrange flow children per `layout` / `direction`".

- [ ] **Step 7: Verify SPEC examples still compile** — extract-and-compile is not automated, so sanity-check by eye that no `layout: row`/`column` remains:

Run: `grep -n 'layout: *row\|layout: *column' SPEC.md`
Expected: no matches.

- [ ] **Step 8: Commit**

```bash
git add SPEC.md
git commit -m "docs(spec): document layout: flow + direction; drop layout:row/column"
```

---

### Task 5: CHARTS.md

**Files:** Modify `CHARTS.md:382-384` (the "planned split" note), and `383`'s `layout: row`/`column` reference.

- [ ] **Step 1: Update the §11 note** — replace lines 382-384:

```markdown
`direction` orients the chart. It is the same property a `flow` uses to pick its
axis (SPEC §5) — a chart adds `radial`:
```

(Keep the table at 386-390 and the rest of §11 unchanged.)

- [ ] **Step 2: Verify**

Run: `grep -n 'layout: *row\|layout: *column\|planned for' CHARTS.md`
Expected: no matches.

- [ ] **Step 3: Commit**

```bash
git add CHARTS.md
git commit -m "docs(charts): direction is now the shared flow/chart orientation (split landed)"
```

---

### Task 6: README.md (palette vars + Mermaid scrub)

**Files:** Modify `README.md:60,61,264` (and any `layout: row` if present — line 78).

- [ ] **Step 1: Palette vars** — `README.md:60`: `link: #444;` → `link: --gray-deep;`. `:61`: `.hot  { fill: #fee; stroke: crimson; }` → `.hot  { fill: --red-wash; stroke: --red-ink; }`. Leave `:184` (`--lini-accent: #ff6600`) — it is the host-page CSS override example where a literal CSS colour is the point.

- [ ] **Step 2: Migrate `layout: row`** — `README.md:78` `layout: row` → `direction: row`.

- [ ] **Step 3: Scrub Mermaid from prose** — `README.md:264` footnote currently names "Mermaid, Graphviz, PlantUML". Reword to drop named tools:

```markdown
<sub>*the common auto-layout diagram generators</sub>
```

- [ ] **Step 4: Verify**

Run: `grep -ni 'mermaid\|#444\|#fee\|crimson\|layout: *row' README.md`
Expected: no matches (the `--lini-accent: #ff6600` line is unaffected).

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs(readme): palette vars over hex; drop named-tool comparison; direction:row"
```

---

### Task 7: Editor syntax highlighting

**Files:** Modify `editors/vscode/syntaxes/lini.tmLanguage.json:109` (template names) and `:159` (keyword values).

- [ ] **Step 1: Add `grid` to the template-name alternation** — line 109, insert `grid` into the `(box|oval|…|row|column|table)` group: `…|row|column|grid|table)`.

- [ ] **Step 2: Add `flow` to the value keywords** — line 159, insert `flow` into the `(grid|row|column|…)` alternation: `(flow|grid|row|column|…)`. (`row`/`column` stay — still valid as `direction` values and `|row|`/`|column|` names.)

- [ ] **Step 3: Validate JSON**

Run: `python3 -c "import json;json.load(open('editors/vscode/syntaxes/lini.tmLanguage.json'))" && echo OK`
Expected: `OK`.

- [ ] **Step 4: Commit**

```bash
git add editors/vscode/syntaxes/lini.tmLanguage.json
git commit -m "editors: highlight layout: flow and the |grid| template"
```

---

### Task 8: Migrate non-chart samples + drop re-applied defaults

**Files:** Modify `samples/*.lini` (non-chart): `layout.lini`, `styles.lini`, `hero.lini`, `icons.lini`, `links.lini`, `links_medium.lini`, `links_simple.lini`, `links_hard.lini`, `desugar.lini`, `templates.lini`, `text.lini`, `gradient.lini`, `palette.lini`, `themes.lini`, `expr.lini`, and others that set `layout:`.

**Reference defaults** (drop when re-applied): root/scene → `layout: flow`, `padding: 20`, `gap: 20`, `font-size: 15`; `|box|` → `padding: 20`, `gap: 20`, `radius: 8`, `stroke-width: 1.5`; `|group|` → `padding: 20`, `gap: 20`, `radius: 8`; `|block|`/`|row|`/`|column|` → `padding: 0`, `gap: 20`. `direction: column` is always a default.

- [ ] **Step 1: Snapshot current render of every sample** (the safety net):

```bash
mkdir -p /tmp/lini-before /tmp/lini-after
for f in samples/*.lini; do cargo run -q -- "$f" -o "/tmp/lini-before/$(basename "$f" .lini).svg" 2>/dev/null; done
```

- [ ] **Step 2: Migrate the keyword** — in each non-chart sample, `layout: row` → `direction: row`; delete redundant `layout: column;` and `layout: flow;`. (e.g. `samples/layout.lini:4` `layout: row; …` keeps `direction: row`; `:9,:27,:46` drop `layout: column;`.)

- [ ] **Step 3: Drop re-applied defaults** — remove properties matching the node's default per the reference above (e.g. a `|group| { … gap: 20 … }` loses `gap: 20`). Be conservative: if unsure whether a value is the default, keep it.

- [ ] **Step 4: Re-render and diff** — output must be byte-identical (dropping a true default changes nothing):

```bash
for f in samples/*.lini; do cargo run -q -- "$f" -o "/tmp/lini-after/$(basename "$f" .lini).svg" 2>/dev/null; done
diff -rq /tmp/lini-before /tmp/lini-after && echo "IDENTICAL"
```

Expected: `IDENTICAL`. Any diff means a dropped property was *not* a default — restore it.

- [ ] **Step 5: Spot-check visually** — render two representative samples to PNG and read them:

```bash
cargo run -q -- samples/layout.lini -o /tmp/layout.svg && resvg /tmp/layout.svg /tmp/layout.png
cargo run -q -- samples/hero.lini -o /tmp/hero.svg && resvg /tmp/hero.svg /tmp/hero.png
```

Read `/tmp/layout.png` and `/tmp/hero.png`; confirm unchanged.

- [ ] **Step 6: Commit**

```bash
git add samples/*.lini
git commit -m "samples: direction:row idiom; drop re-applied defaults (render-identical)"
```

---

### Task 9: Consolidate chart samples (23 → 10) + remove mermaid/outline samples

**Files:**
- Create: `samples/chart_lines.lini`, `samples/chart_points.lini`, `samples/chart_radial.lini`, `samples/chart_axes.lini`, `samples/chart_annotations.lini`.
- Rewrite: `samples/chart_bars.lini`, `samples/chart_pie.lini` (absorb their siblings).
- Keep as-is: `samples/chart_hero.lini`, `samples/chart_fn.lini`, `samples/chart_labels.lini`.
- Delete: `chart_stacked.lini`, `chart_segmented.lini`, `chart_row.lini`, `chart_smooth.lini`, `chart_step.lini`, `chart_area.lini`, `chart_scatter.lini`, `chart_bubbles.lini`, `chart_donut.lini`, `chart_radar.lini`, `chart_radial_bar.lini`, `chart_log.lini`, `chart_reversed.lini`, `chart_dual_axis.lini`, `chart_bands.lini`, `chart_threshold.lini`, `chart_outline.lini`, `mermaid_fast.lini`.

**Merge map:**
| Target | Source charts (copied verbatim, titles kept) |
|---|---|
| `chart_bars` | bars, stacked, segmented, row |
| `chart_lines` | line, smooth, step, area |
| `chart_points` | scatter, bubbles |
| `chart_pie` | pie, donut |
| `chart_radial` | radar, radial_bar |
| `chart_axes` | log, reversed, dual_axis |
| `chart_annotations` | bands, threshold |

**Pattern for a merged file** — wrap the source charts as siblings under a `|grid|` (showcases the new sugar):

```
// Bar-chart variants: grouped, stacked, segmented, horizontal.
|grid| { columns: repeat(2); gap: 24; } [
  <chart block from chart_bars.lini>
  <chart block from chart_stacked.lini>
  <chart block from chart_segmented.lini>
  <chart block from chart_row.lini>
]
```

- [ ] **Step 1: Read each source chart** before deleting — `cat samples/chart_*.lini` — so the merged files reproduce each chart's block verbatim (only the orientation keyword migrated if any uses `layout:`; charts use `direction:` already).

- [ ] **Step 2: Write the 7 merged files** per the map + pattern above.

- [ ] **Step 3: Delete the absorbed sources + `chart_outline.lini` + `mermaid_fast.lini`** (see Files list).

```bash
git rm samples/chart_stacked.lini samples/chart_segmented.lini samples/chart_row.lini \
  samples/chart_smooth.lini samples/chart_step.lini samples/chart_area.lini \
  samples/chart_scatter.lini samples/chart_bubbles.lini samples/chart_donut.lini \
  samples/chart_radar.lini samples/chart_radial_bar.lini samples/chart_log.lini \
  samples/chart_reversed.lini samples/chart_dual_axis.lini samples/chart_bands.lini \
  samples/chart_threshold.lini samples/chart_outline.lini samples/mermaid_fast.lini
```

- [ ] **Step 4: Render every merged chart file to PNG and read it** — confirm each chart renders and the grid lays out cleanly:

```bash
for f in chart_bars chart_lines chart_points chart_pie chart_radial chart_axes chart_annotations; do
  cargo run -q -- "samples/$f.lini" -o "/tmp/$f.svg" && resvg "/tmp/$f.svg" "/tmp/$f.png"
done
```

Read each `/tmp/chart_*.png`; confirm all sub-charts present and legible.

- [ ] **Step 5: Commit**

```bash
git add samples/
git commit -m "samples(charts): consolidate 23 chart samples into 10; drop outline + mermaid"
```

---

### Task 10: Rename the SPEC "Mermaid-fast" section

**Files:** Modify `SPEC.md:1832` (heading) and confirm no other `mermaid` references remain.

- [ ] **Step 1: Rename heading** — `SPEC.md:1832` `### Mermaid-fast` → `### Shorthand — implicit boxes & arrows`. Keep the code example (1834-1840) verbatim.

- [ ] **Step 2: Repo-wide scrub check**

Run: `grep -rni 'mermaid' . --include='*.md' --include='*.lini' --include='*.rs' --include='*.json' | grep -v '/target/'`
Expected: no matches.

- [ ] **Step 3: Commit**

```bash
git add SPEC.md
git commit -m "docs(spec): rename 'Mermaid-fast' to neutral 'Shorthand' section"
```

---

### Task 11: Final verification + version bump

**Files:** Modify `Cargo.toml` (version), `TODO.md` (note) if appropriate.

- [ ] **Step 1: Format, lint, test**

```bash
cargo fmt
cargo clippy --all-targets 2>&1 | tail -15
cargo test 2>&1 | tail -25
```

Expected: clean fmt diff, no clippy warnings, all tests pass.

- [ ] **Step 2: Final keyword sweep** — nothing dropped should remain anywhere:

Run: `grep -rn 'layout: *row\|layout: *column\|LayoutMode::Row\|LayoutMode::Column' . --include='*.rs' --include='*.md' --include='*.lini' | grep -v '/target/'`
Expected: no matches.

- [ ] **Step 3: Bump version** — `Cargo.toml` `version = "0.11.2"` → `version = "0.12.0"` (breaking: `layout:row/column` removed). Run `cargo build` to update `Cargo.lock`.

- [ ] **Step 4: Commit (do not push — defer to user per AGENTS.md)**

```bash
git add Cargo.toml Cargo.lock TODO.md FLOW-DIRECTION.md
git commit -m "chore: bump to v0.12.0 (layout: flow + direction; chart sample consolidation)"
```

---

## Self-Review

- **Spec coverage:** engine/orientation split (T1), sugar incl. `|grid|` (T2), hard-error on old values (T1), test migration (T3), SPEC/CHARTS/README/editor docs (T4-7), sample default-cleanup (T8), chart consolidation + mermaid/outline removal (T9), SPEC mermaid rename (T10), palette vars (T6), verification + version (T11). ✓
- **Placeholders:** none — all Rust steps show full code; doc steps show the replacement prose; sample merges give the pattern + verbatim-copy rule + a byte-identical render gate.
- **Type consistency:** `LayoutMode { Flow, Grid }`, `read_layout_mode -> LayoutMode`, `read_flow_direction -> Axis`, `one_d_dividers(…, Axis, …)` used consistently across T1. `flex::Axis` (`Row`/`Column`) already exists and is `Copy`.
- **Open note:** `read_flow_direction` (layout) and `read_direction` (chart) both parse `direction`; they are intentionally *separate* — different value sets (`radial`) and target enums (`Axis` vs `Dir`) in different modules — not a duplicated mechanism.
