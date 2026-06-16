mod desugar;
mod ir;
mod shapes;
mod styles;
mod vars;

pub use ir::*;

use crate::ast::{
    Attr, AttrItem, BodyItem, DefsBlock, DefsEntry, EndpointGroup, File, LineStyle, SceneConfig,
    ShapeDef, ShapeInst, StyleDef, TypeDefaults, TypeRef, Value, VarOverride, WireConfig, WireDecl,
    WireEndpoint, WireOp,
};
use crate::error::Error;
use crate::span::Span;
use shapes::ShapesTable;
use std::collections::{HashMap, HashSet};
use styles::StyleTable;

#[cfg(test)]
pub fn resolve(file: File) -> Result<Program, Error> {
    resolve_with_theme(file, &[])
}

pub fn resolve_with_theme(file: File, theme: &[(String, String)]) -> Result<Program, Error> {
    // ─── Phase 2.1 — vars & defs setup ───
    let DefTables {
        vars,
        styles: styles_table,
        shapes: shapes_table,
        split,
    } = build_def_tables(&file.defs, theme)?;

    // ─── Phase 2.2 — partition top-level stmts ───
    let (root_nodes, root_wires) = partition_stmts(&file.stmts);

    // ─── Phase 2.3 — resolve scene tree ───
    // Apply scene config to root scene attrs.
    let scene_attrs = match split.scene_config {
        Some(cfg) => {
            let resolved = resolve_attrs(&cfg.items, &styles_table, &vars)?;
            collapse(&resolved)
        }
        None => default_scene_attrs(&vars),
    };

    let mut id_seen: HashMap<String, Span> = HashMap::new();
    let mut scene_nodes = Vec::new();
    let mut internal_wires_lifted: Vec<LiftedWire> = Vec::new();

    // The scene config seeds the cascade for inheritable text attrs.
    let mut root_text_ctx = AttrMap::new();
    for name in INHERITED_TEXT_ATTRS {
        if let Some(v) = scene_attrs.get(name) {
            root_text_ctx.insert(*name, v.clone());
        }
    }

    for inst in &root_nodes {
        let resolved = resolve_inst(
            inst,
            &shapes_table,
            &styles_table,
            &vars,
            &mut id_seen,
            &[],
            &mut internal_wires_lifted,
            &root_text_ctx,
        )?;
        scene_nodes.push(resolved);
    }
    // ─── Phase 2.4 — auto-create, against the expanded tree ───
    // SPEC section 5: a root wire's single-segment endpoint naming an id that
    // exists nowhere in the expanded tree auto-creates an empty |rect| at the
    // scene root. Ids that exist deeper get the did-you-mean error instead.
    let mut path_index = build_path_index(&scene_nodes);
    let mut auto_seen: HashSet<String> = HashSet::new();
    let mut auto_created: Vec<ShapeInst> = Vec::new();
    for wire in &root_wires {
        for group in &wire.chain {
            for ep in &group.endpoints {
                if ep.path.len() != 1 {
                    // Multi-segment paths are navigations, never new nodes.
                    continue;
                }
                let id = &ep.path[0];
                if path_index.has_final_segment(id) || auto_seen.contains(id) {
                    continue;
                }
                auto_seen.insert(id.clone());
                auto_created.push(auto_created_inst(id, ep.span));
            }
        }
    }
    if !auto_created.is_empty() {
        for inst in &auto_created {
            let resolved = resolve_inst(
                inst,
                &shapes_table,
                &styles_table,
                &vars,
                &mut id_seen,
                &[],
                &mut internal_wires_lifted,
                &root_text_ctx,
            )?;
            scene_nodes.push(resolved);
        }
        path_index = build_path_index(&scene_nodes);
    }

    // ─── Phase 2.6 — resolve wires (root + lifted internal) ───
    // Pre-resolve |wire| defaults once — layered as lowest specificity under
    // styles and per-wire attrs.
    let wires_defaults = match split.wire_config {
        Some(cfg) => resolve_attrs(&cfg.items, &styles_table, &vars)?,
        None => Vec::new(),
    };
    let mut wires = Vec::new();
    for w in &root_wires {
        for resolved in resolve_wire(w, &styles_table, &vars, &path_index, &[], &wires_defaults)? {
            wires.push(resolved);
        }
    }
    for lifted in &internal_wires_lifted {
        for resolved in resolve_wire(
            &lifted.wire,
            &styles_table,
            &vars,
            &path_index,
            &lifted.prefix,
            &wires_defaults,
        )? {
            wires.push(resolved);
        }
    }

    // ─── Phase 2.7 — stylesheet inputs for the renderer (SPEC §14) ───
    let sheet = SheetInputs {
        styles: styles_table
            .in_order()
            .into_iter()
            .map(|(name, attrs)| (name, collapse(&attrs)))
            .collect(),
        type_defaults: split
            .type_defaults
            .iter()
            .filter_map(|td| {
                shapes_table
                    .type_default_attrs(&td.name)
                    .map(|attrs| (td.name.clone(), collapse(attrs)))
            })
            .collect(),
        shape_defs: split
            .shape_defs
            .iter()
            .filter_map(|sd| {
                shapes_table
                    .own_attrs(&sd.name)
                    .map(|attrs| (sd.name.clone(), collapse(attrs)))
            })
            .collect(),
        templates: shapes::TEMPLATES
            .iter()
            .map(|(name, _)| (name.to_string(), collapse(&shapes::template_attrs(name))))
            .filter(|(_, attrs)| !attrs.map.is_empty())
            .collect(),
        wire_defaults: collapse(&wires_defaults),
    };

    scene_nodes.retain(|c| !is_blank_anon_text(c));

    Ok(Program {
        vars,
        scene: ResolvedScene {
            attrs: scene_attrs,
            nodes: scene_nodes,
        },
        wires,
        sheet,
    })
}

// ─────────────────────────── Defs partitioning ───────────────────────────

/// Expand a source file's sugar (label → text children with the group
/// caption/footer rules; inline wire labels → wire-text children) into an
/// explicit AST, leaving types, vars, and attrs exactly as written. Used by the
/// `desugar` CLI command to show what a node really means.
pub fn desugar_source(file: &File, theme: &[(String, String)]) -> Result<File, Error> {
    let tables = build_def_tables(&file.defs, theme)?;
    Ok(desugar::desugar_file(file, &tables.shapes))
}

/// The defs-block tables, built once: CSS vars, the style table, the shape
/// table, and the partitioned defs (which borrows `defs`). Shared by full
/// resolution and the `desugar` pass.
struct DefTables<'a> {
    vars: VarTable,
    styles: StyleTable,
    shapes: ShapesTable,
    split: SplitDefs<'a>,
}

fn build_def_tables<'a>(
    defs: &'a Option<DefsBlock>,
    theme: &[(String, String)],
) -> Result<DefTables<'a>, Error> {
    let mut vars = vars::built_in_defaults();
    vars::apply_theme(&mut vars, theme);
    let split = split_defs(defs)?;
    if !split.var_overrides.is_empty() {
        vars::apply_var_overrides(&mut vars, &split.var_overrides)?;
    }
    let styles = StyleTable::build(&split.style_defs, &vars)?;
    let shapes = ShapesTable::build(&split.shape_defs, &split.type_defaults, &styles, &vars)?;
    Ok(DefTables {
        vars,
        styles,
        shapes,
        split,
    })
}

struct SplitDefs<'a> {
    scene_config: Option<&'a SceneConfig>,
    wire_config: Option<&'a WireConfig>,
    type_defaults: Vec<&'a TypeDefaults>,
    var_overrides: Vec<&'a VarOverride>,
    style_defs: Vec<&'a StyleDef>,
    shape_defs: Vec<&'a ShapeDef>,
}

fn split_defs(defs: &Option<DefsBlock>) -> Result<SplitDefs<'_>, Error> {
    let mut scene_config: Option<&SceneConfig> = None;
    let mut wire_config: Option<&WireConfig> = None;
    let mut type_defaults = Vec::new();
    let mut var_overrides = Vec::new();
    let mut style_defs = Vec::new();
    let mut shape_defs = Vec::new();
    if let Some(block) = defs {
        for entry in &block.entries {
            match entry {
                DefsEntry::SceneConfig(s) => {
                    if scene_config.is_some() {
                        return Err(Error::at(
                            s.span,
                            "'|scene|' may appear at most once in the defs block",
                        ));
                    }
                    scene_config = Some(s);
                }
                DefsEntry::WireConfig(w) => {
                    if wire_config.is_some() {
                        return Err(Error::at(
                            w.span,
                            "'|wire|' may appear at most once in the defs block",
                        ));
                    }
                    wire_config = Some(w);
                }
                DefsEntry::TypeDefaults(t) => type_defaults.push(t),
                DefsEntry::VarOverride(v) => var_overrides.push(v),
                DefsEntry::StyleDef(s) => style_defs.push(s),
                DefsEntry::ShapeDef(s) => shape_defs.push(s),
            }
        }
    }
    Ok(SplitDefs {
        scene_config,
        wire_config,
        type_defaults,
        var_overrides,
        style_defs,
        shape_defs,
    })
}

fn default_scene_attrs(vars: &VarTable) -> AttrMap {
    // SPEC §4 default when |scene| is omitted: `layout:row gap:20 padding:20`.
    let mut m = AttrMap::new();
    m.insert("layout", ResolvedValue::Ident("row".into()));
    if let Some(e) = vars.get("gap") {
        m.insert(
            "gap",
            ResolvedValue::LiveVar {
                name: "gap".into(),
                raw: false,
                baked: Some(Box::new(e.value.clone())),
            },
        );
    }
    if let Some(e) = vars.get("canvas-pad") {
        m.insert(
            "padding",
            ResolvedValue::LiveVar {
                name: "canvas-pad".into(),
                raw: false,
                baked: Some(Box::new(e.value.clone())),
            },
        );
    }
    m
}

// ─────────────────────────── Stmt partitioning ───────────────────────────

fn partition_stmts(stmts: &[crate::ast::Stmt]) -> (Vec<ShapeInst>, Vec<WireDecl>) {
    let mut nodes = Vec::new();
    let mut wires = Vec::new();
    for s in stmts {
        match s {
            crate::ast::Stmt::Node(n) => nodes.push(n.clone()),
            crate::ast::Stmt::Wire(w) => wires.push(w.clone()),
        }
    }
    (nodes, wires)
}

// ─────────────────────────── Auto-create ───────────────────────────

fn auto_created_inst(id: &str, span: Span) -> ShapeInst {
    ShapeInst {
        id: Some(id.to_string()),
        ty: TypeRef {
            name: "rect".to_string(),
            span,
        },
        labels: vec![id.to_string()],
        items: Vec::new(),
        body: None,
        span,
    }
}

// ─────────────────────────── Reserved names ───────────────────────────

/// The reserved-identifier error, with the always-available out: idents are
/// case-sensitive, so the capitalized variant is never reserved.
pub(super) fn reserved_error(span: Span, name: &str) -> Error {
    let mut cap = name.to_string();
    if let Some(first) = cap.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    Error::at(
        span,
        format!(
            "'{}' is reserved (ids are case-sensitive — '{}' is free)",
            name, cap
        ),
    )
}

pub(super) fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        // Layout values
        "row" | "column" | "grid"
        | "start" | "center" | "end" | "stretch" | "between" | "around" | "evenly"
        // Edge sides (also endpoint sides) + place values
        | "top" | "bottom" | "left" | "right"
        | "in" | "out"
        | "mid"
        // Primitives
        | "rect" | "oval" | "line" | "path" | "poly" | "text"
        | "hex" | "slant" | "cyl" | "diamond" | "cloud" | "icon" | "image"
        // Templates ("row" is reserved above as a layout value)
        | "group" | "badge" | "note" | "col"
        | "table" | "cell"
        // Defs-only specials
        | "scene" | "wire"
        // Constants
        | "true" | "false" | "none" | "auto"
        // Functions
        | "var" | "rgb" | "rgba" | "hsl"
    )
}

// ─────────────────────────── Attr resolution ───────────────────────────

fn resolve_attrs(
    items: &[AttrItem],
    styles: &styles::StyleTable,
    vars: &VarTable,
) -> Result<Vec<ResolvedAttr>, Error> {
    // SPEC §13: style classes merge in defs-block definition order — listing
    // order is irrelevant, as with CSS classes — and inline attrs merge after
    // every style, regardless of where they sit on the line.
    let mut style_refs: Vec<(usize, &str)> = Vec::new();
    for item in items {
        if let AttrItem::Style(s) = item {
            let idx = styles
                .index(&s.name)
                .ok_or_else(|| Error::at(s.span, format!("unknown style '.{}'", s.name)))?;
            style_refs.push((idx, s.name.as_str()));
        }
    }
    style_refs.sort_by_key(|(idx, _)| *idx);
    style_refs.dedup_by_key(|(idx, _)| *idx);

    let mut out = Vec::new();
    for (_, name) in &style_refs {
        let inner = styles.lookup(name).expect("indexed style expands");
        out.extend(inner.iter().cloned());
    }
    for item in items {
        if let AttrItem::Attr(a) = item {
            out.push(ResolvedAttr {
                name: a.name.clone(),
                value: vars::resolve_value(&a.value, vars)?,
                span: a.span,
            });
        }
    }
    Ok(out)
}

fn collapse(items: &[ResolvedAttr]) -> AttrMap {
    let mut map = AttrMap::new();
    for item in items {
        if is_marker_attr(&item.name) {
            continue;
        }
        map.insert(item.name.clone(), item.value.clone());
    }
    map
}

fn is_marker_attr(name: &str) -> bool {
    matches!(name, "marker" | "marker-start" | "marker-end")
}

// ─────────────────────────── Markers ───────────────────────────

fn resolve_markers(
    items: &[ResolvedAttr],
    default_start: MarkerKind,
    default_end: MarkerKind,
) -> Result<Markers, Error> {
    let mut start = default_start;
    let mut end = default_end;
    for item in items {
        match item.name.as_str() {
            "marker" => {
                let m = expect_marker(&item.value, item.span)?;
                start = m;
                end = m;
            }
            "marker-start" => {
                start = expect_marker(&item.value, item.span)?;
            }
            "marker-end" => {
                end = expect_marker(&item.value, item.span)?;
            }
            _ => {}
        }
    }
    Ok(Markers { start, end })
}

fn expect_marker(value: &ResolvedValue, span: Span) -> Result<MarkerKind, Error> {
    match value {
        ResolvedValue::Ident(s) => MarkerKind::parse(s)
            .ok_or_else(|| Error::at(span, format!("invalid marker value '{}'", s))),
        _ => Err(Error::at(span, "marker attr requires an identifier value")),
    }
}

fn op_markers(op: WireOp) -> Markers {
    Markers {
        start: MarkerKind::from_marker(op.start),
        end: MarkerKind::from_marker(op.end),
    }
}

// ─────────────────────────── Scene tree resolution ───────────────────────────

/// One internal wire (from a shape def body) lifted up to the program level
/// after instantiation, with its endpoint paths prefixed by the instance path.
struct LiftedWire {
    wire: WireDecl,
    /// Dot-path of the host instance (e.g. ["garden"]) — gets prefixed onto
    /// every endpoint path inside the wire at resolution time.
    prefix: Vec<String>,
}

/// Text attrs that cascade from any container to descendant `|text|` nodes —
/// nearest ancestor wins, the node's own attrs win over all (SPEC §11 Text).
const INHERITED_TEXT_ATTRS: &[&str] = &["font", "text-size", "weight", "align"];

fn size_on_text_error(span: Span) -> Error {
    Error::at(span, "'size' is not a text attr; use 'text-size'")
}

#[allow(clippy::too_many_arguments)]
fn resolve_inst(
    inst: &ShapeInst,
    shapes: &shapes::ShapesTable,
    styles_table: &styles::StyleTable,
    vars: &VarTable,
    id_seen: &mut HashMap<String, Span>,
    path_prefix: &[String],
    lifted: &mut Vec<LiftedWire>,
    text_ctx: &AttrMap,
) -> Result<ResolvedInst, Error> {
    let resolved_shape = shapes.resolve(&inst.ty.name, inst.ty.span)?;

    let applied_styles: Vec<String> = inst
        .items
        .iter()
        .filter_map(|i| match i {
            AttrItem::Style(s) => Some(s.name.clone()),
            AttrItem::Attr(_) => None,
        })
        .collect();

    // ID uniqueness + reserved-name check, keyed by full path: siblings in any
    // scope must be distinct (the path index requires it), but the same local
    // id across distinct instances (a.inlet vs b.inlet) has distinct paths and
    // legitimately coexists.
    if let Some(id) = &inst.id {
        if is_reserved(id) {
            return Err(reserved_error(inst.span, id));
        }
        let full = if path_prefix.is_empty() {
            id.clone()
        } else {
            format!("{}.{}", path_prefix.join("."), id)
        };
        if let Some(prev) = id_seen.get(&full) {
            return Err(Error::at(inst.span, format!("duplicate id '{}'", id)).with_related(*prev));
        }
        id_seen.insert(full, inst.span);
    }

    let inline = resolve_attrs(&inst.items, styles_table, vars)?;
    let mut ordered = resolved_shape.attrs.clone();
    ordered.extend(inline);

    // No primitive carries default markers; an "arrow" is just a
    // `|line| marker-end:arrow`. Wires get theirs from the operator.
    let markers = resolve_markers(&ordered, MarkerKind::None, MarkerKind::None)?;
    let mut attrs = collapse(&ordered);

    // `|text|` carries its own label, is sized to its glyphs, and inherits
    // cascaded text attrs; `size` is an error.
    let text_like = resolved_shape.kind == ShapeKind::Text;
    if text_like {
        if attrs.get("size").is_some() {
            return Err(size_on_text_error(inst.span));
        }
        for name in INHERITED_TEXT_ATTRS {
            if attrs.get(name).is_none()
                && let Some(v) = text_ctx.get(name)
            {
                attrs.insert(*name, v.clone());
            }
        }
    }
    // SPEC §8: a slant's skew must stay in the open interval (-89, 89) — at the
    // bounds tan() explodes, shifting the top edge off to infinity.
    if resolved_shape.kind == ShapeKind::Slant
        && let Some(skew) = attrs.number("skew")
        && (skew <= -89.0 || skew >= 89.0)
    {
        return Err(Error::at(
            inst.span,
            format!("skew:{} must be in (-89, 89)", skew),
        ));
    }
    let mut child_text_ctx = text_ctx.clone();
    for name in INHERITED_TEXT_ATTRS {
        if let Some(v) = attrs.get(name) {
            child_text_ctx.insert(*name, v.clone());
        }
    }

    // Compute the dot-path of this inst for nested children.
    let mut child_prefix = path_prefix.to_vec();
    if let Some(id) = &inst.id {
        child_prefix.push(id.clone());
    }

    // Body assembly: shape-def intrinsic children, then label sugar (non-text),
    // then explicit body items from the source.
    let mut body_items: Vec<BodyItem> = resolved_shape.body_items.clone();
    let own_label = if text_like {
        // `|text|` carries its own glyphs; multiple positional strings stack as
        // lines (SPEC §5).
        (!inst.labels.is_empty()).then(|| inst.labels.join("\n"))
    } else {
        // A closed shape's labels become text children (SPEC §5/§9). A `group`
        // is smart about position: the 1st label is a top caption, the 2nd a
        // bottom footer (both reserved bands), the rest plain centred text;
        // every other shape stacks all of them as centred content.
        let is_group = resolved_shape.type_chain.iter().any(|t| t == "group");
        body_items.extend(
            expand_labels(&inst.labels, is_group, inst.span)
                .into_iter()
                .map(BodyItem::Inst),
        );
        None
    };
    if let Some(b) = &inst.body {
        body_items.extend(b.iter().cloned());
    }

    let mut children = Vec::new();
    for item in &body_items {
        match item {
            BodyItem::Inst(child) => {
                children.push(resolve_inst(
                    child,
                    shapes,
                    styles_table,
                    vars,
                    id_seen,
                    &child_prefix,
                    lifted,
                    &child_text_ctx,
                )?);
            }
            BodyItem::Wire(wire) => {
                lifted.push(LiftedWire {
                    wire: wire.clone(),
                    prefix: child_prefix.clone(),
                });
            }
        }
    }
    children.retain(|c| !is_blank_anon_text(c));

    Ok(ResolvedInst {
        id: inst.id.clone(),
        shape: resolved_shape.kind,
        type_chain: resolved_shape.type_chain,
        applied_styles,
        label: own_label,
        attrs,
        markers,
        children,
        span: inst.span,
    })
}

/// A `|text|` instance with no visible content and no id — produced by an empty
/// label (`""`) or a bare `|text| ""`. SPEC §5: `""` suppresses the label, so the
/// node is dropped; left in, it would reserve a band / centred-text slot and emit
/// an empty `<text>`. An id'd empty is kept, so a wire endpoint never dangles.
fn is_blank_anon_text(r: &ResolvedInst) -> bool {
    r.id.is_none() && r.shape == ShapeKind::Text && r.label.as_deref().is_none_or(str::is_empty)
}

/// Expand a shape's positional labels into `|text|` children — the one place
/// the label-sugar rules live, shared by resolution and the `desugar` pass.
pub(super) fn expand_labels(labels: &[String], is_group: bool, span: Span) -> Vec<ShapeInst> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| label_sugar(label, span, is_group, i))
        .collect()
}

/// Desugar one positional label into a `|text|` child. `index` is its position
/// among the host's labels; `is_group` enables the caption/footer promotion.
/// A `group`'s 1st label reserves a top band, its 2nd a bottom band (both at
/// `--title-text-size`); everything else is plain centred text (SPEC §5/§9).
fn label_sugar(text: &str, span: Span, is_group: bool, index: usize) -> ShapeInst {
    let attr = |name: &str, value: Value| {
        AttrItem::Attr(Attr {
            name: name.to_string(),
            value,
            span,
        })
    };
    let band = |side: &str| {
        vec![
            attr("place", Value::Ident("in".into())),
            attr("side", Value::Ident(side.into())),
            attr("text-size", Value::RawCssVar("title-text-size".into())),
        ]
    };
    let items = match (is_group, index) {
        (true, 0) => band("top"),
        (true, 1) => band("bottom"),
        _ => Vec::new(),
    };
    ShapeInst {
        id: None,
        ty: TypeRef {
            name: "text".to_string(),
            span,
        },
        labels: vec![text.to_string()],
        items,
        body: None,
        span,
    }
}

/// Default a wire text's `text-size` to `--wire-text-size` when the author left
/// it unset, so wire labels read a touch smaller than body text.
fn wire_text_attrs(mut map: AttrMap, vars: &VarTable) -> AttrMap {
    if map.get("text-size").is_none() {
        map.insert(
            "text-size".to_string(),
            baked_layout_var(vars, "wire-text-size"),
        );
    }
    map
}

/// A `--name` reference carrying the baked value of a layout var — so it reads
/// as a number at layout time yet still prints as `--name` in live mode.
fn baked_layout_var(vars: &VarTable, name: &str) -> ResolvedValue {
    let baked = match vars.get(name) {
        Some(VarEntry {
            kind: VarKind::Layout,
            value,
        }) => Some(Box::new(value.clone())),
        _ => None,
    };
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
        baked,
    }
}

// ─────────────────────────── Path index ───────────────────────────

/// Maps fully-qualified dot-paths to their place in the scene tree.
struct PathIndex {
    paths: Vec<String>,
}

impl PathIndex {
    fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }

    /// SPEC section 10: an endpoint is an exact path walked from the wire's
    /// scope — the caller prepends the scope prefix. There is no search.
    fn resolve(&self, query: &[String]) -> Option<String> {
        let qjoined = query.join(".");
        self.contains(&qjoined).then_some(qjoined)
    }

    /// Whether any node anywhere in the tree carries this id — the
    /// auto-create gate (only ids absent everywhere materialize).
    fn has_final_segment(&self, seg: &str) -> bool {
        self.paths.iter().any(|p| final_segment(p) == seg)
    }

    /// Full paths of same-named nodes, for did-you-mean errors. Sorted,
    /// capped at 3.
    /// Same-named paths to propose in a wire's scope. For a body wire (non-empty
    /// `scope`) only paths inside that subtree are reachable, and they are
    /// stripped to the form the user types there (`shelf.bowl`, not the
    /// root-absolute `garden.shelf.bowl`). Sorted, deduped, capped at 3.
    fn suggest(&self, seg: &str, scope: &[String]) -> Vec<String> {
        let prefix = if scope.is_empty() {
            String::new()
        } else {
            format!("{}.", scope.join("."))
        };
        let mut hits: Vec<String> = self
            .paths
            .iter()
            .filter(|p| final_segment(p) == seg)
            .filter_map(|p| {
                if prefix.is_empty() {
                    Some(p.clone())
                } else {
                    p.strip_prefix(&prefix).map(str::to_string)
                }
            })
            .collect();
        hits.sort();
        hits.dedup();
        hits.truncate(3);
        hits
    }
}

fn final_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

fn build_path_index(nodes: &[ResolvedInst]) -> PathIndex {
    let mut paths = Vec::new();
    for n in nodes {
        walk_paths(n, &mut Vec::new(), &mut paths);
    }
    PathIndex { paths }
}

fn walk_paths(n: &ResolvedInst, stack: &mut Vec<String>, out: &mut Vec<String>) {
    if let Some(id) = &n.id {
        stack.push(id.clone());
        out.push(stack.join("."));
    }
    for c in &n.children {
        walk_paths(c, stack, out);
    }
    if n.id.is_some() {
        stack.pop();
    }
}

// ─────────────────────────── Wires ───────────────────────────

fn resolve_wire(
    w: &WireDecl,
    styles_table: &styles::StyleTable,
    vars: &VarTable,
    paths: &PathIndex,
    path_prefix: &[String],
    wires_defaults: &[ResolvedAttr],
) -> Result<Vec<ResolvedWire>, Error> {
    let inline = resolve_attrs(&w.items, styles_table, vars)?;
    // Style names ride the wire as `lini-style-*` classes (resolve_attrs above
    // already validated them); their paint comes from the class rules, exactly
    // like a node.
    let applied_styles: Vec<String> = w
        .items
        .iter()
        .filter_map(|i| match i {
            AttrItem::Style(s) => Some(s.name.clone()),
            AttrItem::Attr(_) => None,
        })
        .collect();
    // SPEC section 13 application order: `|wire|` defaults are lowest specificity,
    // styles and per-wire attrs override (the latter are already merged into
    // `inline` left-to-right by `resolve_attrs`).
    let mut ordered: Vec<ResolvedAttr> = Vec::with_capacity(wires_defaults.len() + inline.len());
    ordered.extend(wires_defaults.iter().cloned());
    ordered.extend(inline);

    let op_marks = op_markers(w.op);
    let markers = resolve_markers(&ordered, op_marks.start, op_marks.end)?;
    let mut attrs = collapse(&ordered);

    // Synthesize the line attr for operator variants per SPEC section 10.
    inject_line_style(&mut attrs, w.op.line);

    // Text children: inline labels (distributed along the route by default) +
    // explicit body `|text|`s. Each wire text defaults to `--wire-text-size`.
    let mut texts: Vec<ResolvedText> = Vec::new();
    for label in &w.labels {
        texts.push(ResolvedText {
            text: label.clone(),
            at: WireAt::Auto,
            attrs: wire_text_attrs(AttrMap::new(), vars),
        });
    }
    if let Some(body) = &w.body {
        for t in body {
            let t_attrs = resolve_attrs(&t.items, styles_table, vars)?;
            let mut at = WireAt::Auto;
            let mut t_map = AttrMap::new();
            for item in &t_attrs {
                if item.name == "size" {
                    return Err(size_on_text_error(item.span));
                }
                if item.name == "at" {
                    at = WireAt::parse(&item.value).ok_or_else(|| {
                        Error::at(
                            item.span,
                            "|text| anchor on a wire must be start/mid/end or 0..1",
                        )
                    })?;
                } else if item.name == "place" {
                    // A wire has no inside; only `on` (default) and `out` apply.
                    if matches!(&item.value, ResolvedValue::Ident(s) if s == "in") {
                        return Err(Error::at(
                            item.span,
                            "place:in is not valid on a wire — use place:on (default) or place:out",
                        ));
                    }
                    t_map.insert(item.name.clone(), item.value.clone());
                } else {
                    t_map.insert(item.name.clone(), item.value.clone());
                }
            }
            texts.push(ResolvedText {
                text: t.text.clone(),
                at,
                attrs: wire_text_attrs(t_map, vars),
            });
        }
    }

    // Cartesian fan expansion: each group's endpoints fan out independently.
    // For chain [{a}, {b,c}, {d}] with op `->`, we get a→b→d, a→c→d (each as
    // its own wire). Per spec section 10 wire fan grammar.
    let expanded = expand_chain(&w.chain);

    let mut out = Vec::with_capacity(expanded.len());
    for (fan_index, chain_path) in expanded.into_iter().enumerate() {
        let mut endpoints = Vec::with_capacity(chain_path.len());
        for ep in chain_path {
            let qualified: Vec<String> = if path_prefix.is_empty() {
                ep.path.clone()
            } else {
                // For internal wires lifted from a shape body, prefix the
                // endpoint with the host inst's id-path before resolution.
                let mut p = path_prefix.to_vec();
                p.extend(ep.path.iter().cloned());
                p
            };
            let resolved_path = match paths.resolve(&qualified) {
                Some(p) => p,
                None => {
                    let scope = if path_prefix.is_empty() {
                        "at scene root".to_string()
                    } else {
                        format!("in '{}'", path_prefix.join("."))
                    };
                    let mut msg =
                        format!("wire endpoint '{}' not found {}", ep.path.join("."), scope);
                    let suggestions =
                        paths.suggest(ep.path.last().expect("non-empty path"), path_prefix);
                    if !suggestions.is_empty() {
                        let quoted: Vec<String> =
                            suggestions.iter().map(|s| format!("'{}'", s)).collect();
                        msg.push_str(&format!("; did you mean {}?", quoted.join(", ")));
                    }
                    return Err(Error::at(ep.span, msg));
                }
            };
            endpoints.push(ResolvedEndpoint {
                path: resolved_path,
                side: ep.side,
                span: ep.span,
            });
        }
        out.push(ResolvedWire {
            endpoints,
            attrs: attrs.clone(),
            applied_styles: applied_styles.clone(),
            markers: markers.clone(),
            // A fan declaration's label is written once; cartesian expansion
            // would otherwise copy it onto every sibling.
            texts: if fan_index == 0 {
                texts.clone()
            } else {
                Vec::new()
            },
            span: w.span,
        });
    }
    Ok(out)
}

fn inject_line_style(attrs: &mut AttrMap, line: LineStyle) {
    let style = match line {
        LineStyle::Solid => return,
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
        // wavy isn't first-class in the renderer yet — tagged so render can
        // branch later.
        LineStyle::Wavy => "wavy",
    };
    // Don't override an explicit line attr.
    if attrs.get("line").is_none() {
        attrs.insert("line", ResolvedValue::Ident(style.into()));
    }
}

/// Take a wire chain and expand the cartesian fan-out across endpoint groups.
/// Result: each entry is one fully-flattened endpoint sequence (one wire).
fn expand_chain(chain: &[EndpointGroup]) -> Vec<Vec<WireEndpoint>> {
    let mut acc: Vec<Vec<WireEndpoint>> = vec![Vec::new()];
    for group in chain {
        let mut next: Vec<Vec<WireEndpoint>> =
            Vec::with_capacity(acc.len() * group.endpoints.len());
        for trail in &acc {
            for ep in &group.endpoints {
                let mut t = trail.clone();
                t.push(ep.clone());
                next.push(t);
            }
        }
        acc = next;
    }
    acc
}

// ─────────────────────────── Tests ───────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_str(src: &str) -> Program {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::parser::parse(&tokens).expect("parse");
        resolve(file).expect("resolve")
    }

    fn resolve_err(src: &str) -> Error {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::parser::parse(&tokens).expect("parse");
        match resolve(file) {
            Ok(_) => panic!("expected resolve error"),
            Err(e) => e,
        }
    }

    #[test]
    fn single_letter_ids_are_legal() {
        let p = resolve_str("a -> b\n");
        let ids: Vec<&str> = p
            .scene
            .nodes
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert!(ids.contains(&"a") && ids.contains(&"b"));
    }

    #[test]
    fn short_side_forms_are_plain_segments() {
        let e = resolve_err("p |rect|\nq |rect|\np.r -> q\n");
        assert!(e.message.contains("not found"), "got: {}", e.message);
    }

    #[test]
    fn long_side_forms_parse() {
        let p = resolve_str("p |rect|\nq |rect|\np -> q.left\n");
        assert_eq!(p.wires[0].endpoints[1].side, Some(crate::ast::Side::Left));
    }

    #[test]
    fn bare_name_no_longer_reaches_into_containers() {
        let e = resolve_err("kitchen |group| { inlet |rect|\noutlet |rect| }\noutlet -> inlet\n");
        assert!(
            e.message.contains("did you mean 'kitchen.outlet'"),
            "got: {}",
            e.message
        );
    }

    #[test]
    fn template_internals_error_with_suggestions_not_phantoms() {
        let e = resolve_err(
            "{ |room:group| { inlet |rect|\noutlet |rect| } }\n\
             closet |room|\nfridge |room|\noutlet -> inlet\n",
        );
        assert!(
            e.message.contains("closet.outlet") && e.message.contains("fridge.outlet"),
            "got: {}",
            e.message
        );
    }

    #[test]
    fn full_paths_resolve() {
        let p = resolve_str(
            "{ |room:group| { inlet |rect|\noutlet |rect| } }\n\
             closet |room|\nfridge |room|\ncloset.outlet -> fridge.inlet\n",
        );
        assert_eq!(p.wires[0].endpoints[0].path, "closet.outlet");
        assert_eq!(p.wires[0].endpoints[1].path, "fridge.inlet");
    }

    #[test]
    fn body_wire_sees_siblings() {
        let p = resolve_str("garden |group| { a1 |rect|\nb1 |rect|\na1 -> b1 }\n");
        assert_eq!(p.wires[0].endpoints[0].path, "garden.a1");
        assert_eq!(p.wires[0].endpoints[1].path, "garden.b1");
    }

    #[test]
    fn body_wire_cannot_see_out() {
        let e = resolve_err("outsider |rect|\ngarden |group| { a1 |rect|\na1 -> outsider }\n");
        assert!(e.message.contains("not found"), "got: {}", e.message);
    }

    #[test]
    fn body_wires_never_autocreate() {
        let e = resolve_err("garden |group| { a1 |rect|\na1 -> ghost }\n");
        assert!(e.message.contains("not found"), "got: {}", e.message);
    }

    #[test]
    fn typo_autocreates_when_absent_everywhere() {
        let p = resolve_str("alpha |rect|\nalpha -> betta\n");
        assert!(
            p.scene
                .nodes
                .iter()
                .any(|n| n.id.as_deref() == Some("betta"))
        );
    }

    #[test]
    fn deep_grandchild_needs_full_path_from_scope() {
        let e = resolve_err(
            "kitchen |group| { counter |group| { bowl |rect| } }\nkitchen.bowl -> kitchen\n",
        );
        assert!(
            e.message.contains("kitchen.counter.bowl"),
            "got: {}",
            e.message
        );
    }

    #[test]
    fn style_definition_order_decides() {
        for node in ["x |rect| .a .b\n", "x |rect| .b .a\n"] {
            let src = format!("{{ .a stroke:red\n  .b stroke:blue }}\n{}", node);
            let p = resolve_str(&src);
            let got = format!("{:?}", p.scene.nodes[0].attrs.get("stroke"));
            assert!(got.contains("blue"), "node {:?} → {}", node, got);
        }
    }

    #[test]
    fn inline_attrs_beat_styles_regardless_of_position() {
        let p = resolve_str("{ .a stroke:red }\nx |rect| stroke:green .a\n");
        let got = format!("{:?}", p.scene.nodes[0].attrs.get("stroke"));
        assert!(got.contains("green"), "got {}", got);
    }

    #[test]
    fn text_size_cascades_from_container() {
        let p = resolve_str("g |group| text-size:10 { t |text| \"hi\" }\n");
        let txt = &p.scene.nodes[0].children[0];
        assert_eq!(txt.attrs.number("text-size"), Some(10.0));
    }

    #[test]
    fn nearest_text_size_wins() {
        let p =
            resolve_str("g |group| text-size:10 { h |group| text-size:20 { t |text| \"hi\" } }\n");
        let txt = &p.scene.nodes[0].children[0].children[0];
        assert_eq!(txt.attrs.number("text-size"), Some(20.0));
    }

    #[test]
    fn own_text_attrs_beat_inherited() {
        let p = resolve_str("g |group| text-size:10 { t |text| \"hi\" text-size:8 }\n");
        let txt = &p.scene.nodes[0].children[0];
        assert_eq!(txt.attrs.number("text-size"), Some(8.0));
    }

    #[test]
    fn font_cascades_to_label_sugar() {
        let p = resolve_str("box |rect| \"Label\" font:serif\n");
        let txt = &p.scene.nodes[0].children[0];
        match txt.attrs.get("font") {
            Some(ResolvedValue::Ident(s)) => assert_eq!(s, "serif"),
            other => panic!("expected font=serif on sugar text, got {:?}", other),
        }
    }

    #[test]
    fn scene_text_size_reaches_all_text() {
        let p = resolve_str("{ |scene| text-size:15 }\nt |text| \"hi\"\n");
        let txt = &p.scene.nodes[0];
        assert_eq!(txt.attrs.number("text-size"), Some(15.0));
    }

    #[test]
    fn size_on_text_errors_with_hint() {
        let e = resolve_err("t |text| \"x\" size:11\n");
        assert!(e.message.contains("use 'text-size'"), "got: {}", e.message);
    }

    #[test]
    fn size_on_wire_text_errors_with_hint() {
        let e = resolve_err("a |rect|\nz |rect|\na -> z { |text| \"x\" size:9 }\n");
        assert!(e.message.contains("use 'text-size'"), "got: {}", e.message);
    }

    #[test]
    fn wire_text_accepts_style_refs() {
        let p = resolve_str(
            "{ .small text-size:9 }\na |rect|\nz |rect|\na -> z { |text| \"hi\" .small }\n",
        );
        match p.wires[0].texts[0].attrs.get("text-size") {
            Some(ResolvedValue::Number(n)) => assert_eq!(*n, 9.0),
            other => panic!(
                "expected text-size=9 from the .small style, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn dashed_op_injects_line_attr() {
        let p = resolve_str("a |rect|\nz |rect|\na --> z\n");
        match p.wires[0].attrs.get("line") {
            Some(ResolvedValue::Ident(s)) => assert_eq!(s, "dashed"),
            other => panic!("expected line=dashed, got {:?}", other),
        }
    }

    #[test]
    fn explicit_line_attr_beats_operator() {
        let p = resolve_str("a |rect|\nz |rect|\na --> z line:dotted\n");
        match p.wires[0].attrs.get("line") {
            Some(ResolvedValue::Ident(s)) => assert_eq!(s, "dotted"),
            other => panic!("expected line=dotted, got {:?}", other),
        }
    }

    #[test]
    fn reserved_id_error_hints_capitalized_variant() {
        let e = resolve_err("start |rect|\n");
        assert!(e.message.contains("'Start' is free"), "got: {}", e.message);
    }

    #[test]
    fn marker_order_marker_before_marker_end() {
        let p = resolve_str(
            "cat |rect| \"Cat\"\n\
             dog |rect| \"Dog\"\n\
             cat -> dog marker:arrow marker-end:dot\n",
        );
        let w = &p.wires[0];
        assert_eq!(w.markers.start, MarkerKind::Arrow);
        assert_eq!(w.markers.end, MarkerKind::Dot);
    }

    #[test]
    fn wire_op_default_markers() {
        let p = resolve_str(
            "cat |rect| \"Cat\"\n\
             dog |rect| \"Dog\"\n\
             cat <-> dog\n",
        );
        let w = &p.wires[0];
        assert_eq!(w.markers.start, MarkerKind::Arrow);
        assert_eq!(w.markers.end, MarkerKind::Arrow);
    }

    #[test]
    fn defaults_override_layout_var_keeps_kind_and_bakes_value() {
        let p = resolve_str("{ --gap:30 }\nx |rect|\n");
        let entry = p.vars.get("gap").expect("gap present");
        assert_eq!(entry.kind, VarKind::Layout);
        match &entry.value {
            ResolvedValue::Number(n) => assert_eq!(*n, 30.0),
            other => panic!("expected Number(30), got {:?}", other),
        }
    }

    #[test]
    fn label_sugar_creates_text_child_on_non_text_shape() {
        let p = resolve_str("cat |rect| \"hello\"\n");
        let r = &p.scene.nodes[0];
        assert_eq!(r.shape, ShapeKind::Rect);
        assert!(r.label.is_none(), "non-text shape keeps no label");
        assert_eq!(r.children.len(), 1);
        let t = &r.children[0];
        assert_eq!(t.shape, ShapeKind::Text);
        assert_eq!(t.label.as_deref(), Some("hello"));
    }

    #[test]
    fn empty_label_suppresses_the_text_child() {
        // SPEC §5: `""` suppresses the label — no centred text, no title band,
        // and no empty `<text>` left to reserve layout space.
        let rect = resolve_str("cat |rect| \"\"\n");
        assert!(rect.scene.nodes[0].children.is_empty(), "no sugar child");

        let group = resolve_str("g |group| \"\" {\n  x |rect| \"X\"\n}\n");
        assert!(
            !group.scene.nodes[0]
                .children
                .iter()
                .any(|c| c.shape == ShapeKind::Text),
            "no phantom caption band"
        );

        let bare = resolve_str("|text| \"\"\n");
        assert!(bare.scene.nodes.is_empty(), "blank bare text is dropped");
    }

    #[test]
    fn text_label_stays_on_text_inst() {
        let p = resolve_str("cat |text| \"hello\"\n");
        let t = &p.scene.nodes[0];
        assert_eq!(t.shape, ShapeKind::Text);
        assert_eq!(t.label.as_deref(), Some("hello"));
        assert!(t.children.is_empty());
    }

    #[test]
    fn shape_inheritance_resolves_to_primitive_kind() {
        let p = resolve_str("{ |treat:rect| radius:5 }\ncat |treat| \"Cat\"\n");
        let n = &p.scene.nodes[0];
        assert_eq!(n.shape, ShapeKind::Rect);
        assert!(n.attrs.get("radius").is_some());
    }

    #[test]
    fn wire_auto_creates_undeclared_endpoints() {
        let p = resolve_str("cat -> dog\n");
        // Both `cat` and `dog` auto-created as rects.
        assert_eq!(p.scene.nodes.len(), 2);
        let ids: Vec<&str> = p
            .scene
            .nodes
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert!(ids.contains(&"cat"));
        assert!(ids.contains(&"dog"));
    }

    #[test]
    fn wire_fan_expands_cartesian() {
        let p = resolve_str("cat & fox -> bird & mouse\n");
        assert_eq!(p.wires.len(), 4);
    }

    #[test]
    fn fan_label_is_not_duplicated_across_siblings() {
        // `a -> b & c "shared"` expands to two wires; the one declared label
        // rides a single sibling (E2 — drawn once), not each of them.
        let p = resolve_str(
            "src |rect|\n\
             one |rect|\n\
             two |rect|\n\
             src -> one & two \"shared\"\n",
        );
        assert_eq!(p.wires.len(), 2);
        let labelled = p.wires.iter().filter(|w| !w.texts.is_empty()).count();
        assert_eq!(labelled, 1, "the fan label rides one sibling, not each");
    }
}
