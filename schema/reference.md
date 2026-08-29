# Lini property reference

Generated from the property ledger by `cargo xtask gen-schema` — **do not edit**. Schema v1, lini 1.0.0-beta.5. The machine-readable form, with one compiled example per property, is `lini.schema.json`.

## Primitives

`block` `oval` `hex` `slant` `cyl` `diamond` `poly` `path` `text` `line` `icon` `image` `sketch`

## Templates

| template | primitive | chain |
|---|---|---|
| `box` | `block` | `box` |
| `rect` | `block` | `box` → `rect` |
| `group` | `block` | `group` |
| `caption` | `block` | `caption` |
| `footnote` | `block` | `caption` → `footnote` |
| `badge` | `block` | `badge` |
| `row` | `block` | `row` |
| `column` | `block` | `column` |
| `grid` | `block` | `grid` |
| `table` | `block` | `group` → `table` |
| `cell` | `block` | `cell` |
| `header` | `block` | `cell` → `header` |
| `footer` | `block` | `cell` → `footer` |
| `entity` | `block` | `group` → `table` → `entity` |
| `sign` | `icon` | `sign` |
| `chart` | `block` | `chart` |
| `pie` | `block` | `pie` |
| `area` | `block` | `area` |
| `bars` | `block` | `bars` |
| `dots` | `block` | `dots` |
| `bubble` | `block` | `bubble` |
| `slice` | `block` | `slice` |
| `axis` | `block` | `axis` |
| `band` | `block` | `band` |
| `mark` | `block` | `mark` |
| `sequence` | `block` | `sequence` |
| `loop` | `block` | `loop` |
| `opt` | `block` | `opt` |
| `alt` | `block` | `alt` |
| `else` | `block` | `else` |
| `note` | `block` | `note` |
| `balloon` | `oval` | `balloon` |
| `topic` | `block` | `topic` |
| `mindmap` | `block` | `topic` → `mindmap` |
| `drawing` | `block` | `drawing` |
| `hole` | `oval` | `hole` |
| `centerline` | `line` | `centerline` |
| `pitch-circle` | `oval` | `pitch-circle` |
| `breakline` | `line` | `breakline` |
| `hidden` | `sketch` | `hidden` |
| `shoulder` | `line` | `shoulder` |
| `threadline` | `line` | `threadline` |
| `halo` | `line` | `halo` |
| `surface-finish` | `block` | `surface-finish` |
| `feature-control` | `block` | `feature-control` |
| `control` | `block` | `control` |
| `datum` | `block` | `datum` |
| `plane` | `line` | `plane` |
| `magnifier` | `oval` | `magnifier` |
| `projection` | `line` | `projection` |
| `page` | `block` | `page` |
| `title-block` | `block` | `group` → `table` → `title-block` |
| `field` | `block` | `field` |
| `frame` | `block` | `box` → `rect` → `frame` |
| `zone` | `block` | `zone` |
| `tick` | `line` | `tick` |
| `floorplan` | `block` | `drawing` → `floorplan` |
| `wall` | `sketch` | `wall` |
| `partition` | `sketch` | `wall` → `partition` |
| `door` | `block` | `door` |
| `window` | `block` | `window` |
| `door-leaf` | `line` | `door-leaf` |
| `door-swing` | `line` | `door-swing` |
| `window-sill` | `line` | `window-sill` |
| `bed` | `block` | `bed` |
| `sofa` | `block` | `sofa` |
| `dining` | `block` | `dining` |
| `bath` | `block` | `bath` |
| `appliance` | `block` | `appliance` |
| `stairs` | `block` | `stairs` |
| `schematic` | `block` | `schematic` |
| `component` | `block` | `component` |
| `pin` | `block` | `pin` |
| `label` | `block` | `label` |
| `junction` | `oval` | `junction` |
| `J` | `block` | `component` → `J` |
| `opamp` | `block` | `component` → `opamp` |
| `gnd` | `block` | `label` → `gnd` |
| `nc` | `block` | `label` → `nc` |
| `R` | `block` | `R` |
| `C` | `block` | `C` |
| `L` | `block` | `L` |
| `D` | `block` | `D` |
| `LED` | `block` | `LED` |
| `Q` | `block` | `Q` |
| `Y` | `block` | `Y` |
| `F` | `block` | `F` |
| `FB` | `block` | `FB` |
| `SW` | `block` | `SW` |
| `BT` | `block` | `BT` |
| `V` | `block` | `V` |
| `I` | `block` | `I` |

## Layout engines

`flow` `grid` `sequence` `chart` `tree` `pie` `schematic`

## Roles

`discrete` `closed` `series` `mate` `opening` `dimension` `title-block`

## Value builders

`oklch` `gradient` `linear-gradient` `radial-gradient` `rgb` `rgba` `hsl` `hsla` `repeat` `hatch`

## Properties

Shape is `form:kind` (see `enums` in the JSON). Flags: `text` valid on a bare text leaf · `baked` compiled into positions, never live CSS · `hard-gate` errors out of scope · `deferred` named but not built — writing it errors ([SPEC 24](../SPEC.md#24-deferred)) · `dual-channel` cascades two ways (`format`).

| property | owners | shape | default | inherit | flags |
|---|---|---|---|---|---|
| `fill` | universal | `one:paint` | `bundles` | — | text |
| `opacity` | universal | `one:number` | `engine` | — | text |
| `stroke` | universal | `one:paint` | `bundles` | — | — |
| `stroke-width` | universal | `one:number` | `bundles` | — | — |
| `stroke-style` | universal | `one:ident` | `bundles` | — | — |
| `radius` | universal | `one:number` | `bundles` | — | — |
| `shadow` | universal | `one:any` | `none` | — | — |
| `gap-fill` | flow (layout), grid (layout), sequence (layout) | `one:paint` | `bundles` | — | — |
| `font-family` | universal | `one:any` | `engine` | text | text |
| `font-size` | universal | `one:number` | `bundles` | text | text baked |
| `font-weight` | universal | `one:any` | `bundles` | text | text |
| `font-style` | universal | `one:ident` | `engine` | text | text |
| `text-transform` | universal | `one:ident` | `engine` | text | text |
| `text-decoration` | universal | `one:ident` | `engine` | text | text |
| `text-shadow` | universal | `one:any` | `none` | text | text |
| `letter-spacing` | universal | `one:number` | `engine` | text | text baked |
| `line-spacing` | universal | `one:number` | `engine` | text | text baked |
| `color` | universal | `one:colour` | `engine` | text | text |
| `width` | universal | `one:any` | `engine` | — | — |
| `height` | universal | `one:any` | `engine` | — | — |
| `padding` | universal | `one:number` | `bundles` | — | — |
| `max-width` | universal | `one:number` | `none` | — | — |
| `text-wrap` | universal | `one:ident` | `none` | — | — |
| `pin` | universal | `one:ident` | `engine` | — | — |
| `translate` | universal | `one:number` | `none` | — | text |
| `rotate` | universal | `one:number` | `engine` | — | text |
| `layer` | universal | `one:number` | `engine` | — | text |
| `scale` | universal, \|axis\| | `one:any` | `bundles` | engine | — |
| `pattern` | universal | `pattern` | `none` | — | — |
| `mirror` | universal | `one:any` | `none` | — | — |
| `href` | universal, link | `one:string` | `none` | — | — |
| `hint` | universal | `one:string` | `none` | — | — |
| `points` | \|line\|, \|poly\| | `list:number` | `none` | — | — |
| `samples` | \|line\|, \|poly\|, \|chart\| | `one:number` | `engine` | — | — |
| `path` | \|path\| | `one:string` | `none` | — | — |
| `src` | \|image\| | `one:string` | `none` | — | — |
| `symbol` | \|icon\|, \|surface-finish\|, \|label\|, discrete (role), \|door\|, \|bed\|, \|sofa\|, \|dining\|, \|bath\|, \|appliance\| | `one:ident` | `none` | — | — |
| `fit` | \|icon\|, \|image\| | `one:ident` | `bundles` | — | — |
| `skew` | \|slant\| | `one:number` | `bundles` | — | — |
| `stack` | closed (role) | `one:number` | `none` | — | — |
| `marker` | \|line\|, \|mark\|, series (role), link | `one:marker` | `engine` | — | — |
| `marker-start` | \|line\|, \|mark\|, series (role), link | `one:marker` | `engine` | — | — |
| `marker-end` | \|line\|, \|mark\|, series (role), link | `one:marker` | `engine` | — | — |
| `draw` | \|sketch\| | `pen` | `none` | — | — |
| `revolve` | \|sketch\| | `one:ident` | `none` | — | — |
| `thread` | \|sketch\|, \|hole\| | `list:any` | `none` | — | — |
| `sheet` | \|page\| | `one:any` | `engine` | — | — |
| `break` | \|sketch\| | `list:any` | `none` | — | — |
| `layout` | universal | `one:ident` | `bundles` | — | — |
| `direction` | flow (layout), chart (layout), tree (layout) | `one:ident` | `engine` | — | — |
| `gap` | flow (layout), grid (layout), sequence (layout), chart (layout), pie (layout), tree (layout), schematic (layout), mate (role) | `one:number` | `bundles` | — | — |
| `align` | flow (layout), grid (layout) | `list:ident` | `bundles` | — | — |
| `justify` | flow (layout), grid (layout) | `list:ident` | `bundles` | — | — |
| `columns` | grid (layout), schematic (layout) | `list:track` | `bundles` | — | — |
| `rows` | grid (layout) | `list:track` | `none` | — | — |
| `cell` | grid (layout), schematic (layout) | `one:number` | `none` | — | hard-gate |
| `span` | grid (layout) | `one:number` | `none` | — | hard-gate |
| `data` | series (role) | `list:number` | `none` | — | — |
| `fn` | series (role) | `list:any` | `none` | — | — |
| `labels` | series (role) | `list:string` | `none` | — | — |
| `curve` | \|line\|, \|area\| | `one:ident` | `engine` | — | — |
| `baseline` | \|area\| | `one:number` | `engine` | — | — |
| `axis` | series (role), \|mark\|, \|band\| | `one:ident` | `none` | — | — |
| `bars` | \|chart\| | `one:ident` | `engine` | — | — |
| `categories` | \|chart\| | `list:string` | `engine` | — | — |
| `hole` | \|pie\| | `one:number` | `engine` | — | — |
| `legend` | \|chart\|, \|pie\| | `one:any` | `engine` | — | deferred |
| `tooltip` | \|chart\|, \|pie\|, series (role) | `one:any` | `engine` | — | — |
| `value` | \|slice\|, \|bubble\| | `one:number` | `none` | — | — |
| `at` | \|mark\|, \|bubble\|, \|plane\|, opening (role) | `one:any` | `none` | — | — |
| `side` | \|axis\|, \|topic\|, \|pin\|, \|label\|, link, dimension (role) | `one:ident` | `engine` | — | — |
| `range` | \|axis\|, \|band\| | `one:number` | `none` | — | — |
| `step` | \|axis\| | `one:number` | `none` | — | — |
| `ticks` | \|axis\| | `list:number` | `none` | — | — |
| `unit` | \|drawing\|, \|axis\| | `one:any` | `none` | — | — |
| `gridlines` | \|axis\| | `one:any` | `engine` | — | — |
| `format` | \|chart\|, \|pie\|, \|axis\|, series (role), \|drawing\|, dimension (role) | `one:any` | `engine` | scope-link | dual-channel |
| `place` | \|note\| | `one:any` | `none` | — | hard-gate |
| `activation` | sequence (layout) | `one:ident` | `engine` | — | hard-gate |
| `tol` | dimension (role), \|feature-control\|, \|control\| | `one:any` | `none` | — | hard-gate |
| `characteristic` | \|feature-control\|, \|control\| | `one:ident` | `none` | — | — |
| `zone` | \|feature-control\|, \|control\| | `one:ident` | `none` | — | — |
| `material` | \|feature-control\|, \|control\| | `one:ident` | `none` | — | — |
| `datums` | \|feature-control\|, \|control\| | `list:any` | `none` | — | — |
| `modifiers` | \|feature-control\|, \|control\| | `list:any` | `none` | — | — |
| `project` | dimension (role) | `one:ident` | `none` | — | hard-gate |
| `facing` | \|plane\| | `one:ident` | `engine` | — | — |
| `of` | \|drawing\| | `one:ident` | `none` | — | — |
| `title` | title-block (role) | `one:string` | `none` | — | — |
| `drawing-number` | title-block (role) | `one:string` | `none` | — | — |
| `revision` | title-block (role) | `one:string` | `none` | — | — |
| `sheet-number` | title-block (role) | `one:string` | `none` | — | — |
| `date` | title-block (role) | `one:string` | `none` | — | — |
| `author` | title-block (role) | `one:string` | `none` | — | — |
| `approved` | title-block (role) | `one:string` | `none` | — | — |
| `department` | title-block (role) | `one:string` | `none` | — | — |
| `reference` | title-block (role) | `one:string` | `none` | — | — |
| `document-type` | title-block (role) | `one:string` | `none` | — | — |
| `status` | title-block (role) | `one:string` | `none` | — | — |
| `density` | root | `one:number` | `engine` | — | — |
| `thickness` | \|floorplan\|, \|wall\| | `one:number` | `engine` | engine | — |
| `on` | opening (role) | `one:ident` | `none` | — | — |
| `hinge` | \|door\| | `one:ident` | `engine` | — | — |
| `swing` | \|door\| | `one:ident` | `engine` | — | — |
| `steps` | \|stairs\| | `one:number` | `none` | — | — |
| `number` | \|pin\| | `one:number` | `none` | — | — |
| `prefix` | \|component\|, discrete (role) | `one:string` | `bundles` | — | — |
| `shape` | \|label\| | `one:ident` | `bundles` | — | — |
| `pins` | \|J\| | `one:number` | `none` | — | — |
| `clearance` | link, root, dimension (role) | `one:number` | `bundles` | scope-link | — |
| `routing` | root | `one:ident` | `engine` | scope-link | — |
| `along` | link | `list:number` | `engine` | — | — |
| `corner-radius` | link | `one:any` | `engine` | — | — |
