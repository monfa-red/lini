//! The resolve orchestrator: variables → stylesheet → scene tree → links →
//! render inputs, assembled into a [`Program`] [SPEC 18]. Types, templates,
//! defines, labels, and auto-create are lowered upstream by `desugar`, so resolve
//! only ever sees primitives and `.lini-*` classes.
//!
//! [`lib.rs`]'s compile pipeline enters resolution here (after `desugar`).

use super::cascade::{NodeFacts, Stylesheet};
use super::defaults;
use super::ir::{
    AttrMap, Program, ResolvedCall, ResolvedInst, ResolvedScene, ResolvedValue, SheetInputs,
    VarTable, is_drawing,
};
use super::links;
use super::merge::collapse;
use super::scene::{self, PathIndex, SceneCtx};
use super::value::{resolve_groups, resolve_property};
use crate::error::Error;
use crate::expr::{Expr, FuncTable};
use crate::ledger::{consts, properties};
use crate::syntax::ast::{Decl, File, Rule, SelUnit, StyleItem};
use std::collections::{HashMap, HashSet};

mod link_scope;
mod theme;

use link_scope::{baked_link_defaults, link_scope_kind};
use theme::{apply_theme, apply_var_decls, build_funcs};

#[cfg(test)]
mod tests;

/// Resolve a parsed file into a [`Program`].
pub fn resolve(file: &File, theme: &[(String, String)]) -> Result<Program, Error> {
    // ── Variables: built-in visual-var defaults ← theme ← `--name` decls ──
    let mut vars = defaults::built_in_defaults();
    apply_theme(&mut vars, theme);

    // ── Functions: parse each `name(params) `body`` and reject cycles [SPEC 10.7];
    //    every numeric value folds against this table. ──
    let funcs = build_funcs(file)?;
    apply_var_decls(&mut vars, file, &funcs)?;

    // ── Stylesheet: the desugared file's rules (generated `.lini-*` type
    //    classes, engine-supplied scoped rules, descendant + user-class
    //    rules) — desugar owns every generated rule [SPEC 8/18]. ──
    let rules: Vec<&Rule> = file
        .stylesheet
        .iter()
        .filter_map(|it| match it {
            StyleItem::Rule(r) => Some(r),
            _ => None,
        })
        .collect();
    let sheet = Stylesheet::build(&rules, &vars, &funcs)?;

    // ── Root configuration + the text props it cascades ──
    let root_attrs = root_attrs(file, &vars, &funcs)?;
    let mut root_text_ctx = AttrMap::new();
    for name in properties::inherited_text() {
        if let Some(v) = root_attrs.get(name) {
            root_text_ctx.insert(name, v.clone());
        }
    }

    // ── Scene tree (types/templates/defines, labels, and auto-create were all
    //    lowered by desugar — resolve only sees primitives + classes) ──
    let ctx = SceneCtx {
        sheet: &sheet,
        vars: &vars,
        funcs: &funcs,
    };
    let mut id_seen = HashMap::new();
    let mut lifted = Vec::new();
    let nodes = scene::resolve_instances(
        &file.instances,
        &ctx,
        &root_attrs,
        &root_text_ctx,
        &mut id_seen,
        &mut lifted,
    )?;
    let index = PathIndex::build(&nodes);

    // ── Links: root statements then lifted internal links. A link resolves through
    //    the node cascade [SPEC 9, 4]: `resolve_link` layers the `|-|` rules over
    //    a base of the baked defaults plus the scope's `clearance` / `routing`, and
    //    matches descendant rules against the scope's container chain. ──
    let baked = baked_link_defaults(&vars, &funcs)?;
    // The containment-link cascade [SPEC 9]: a link whose endpoints are X and
    // X.path cascades its descendant rules as if written in X — this lookup
    // gives `resolve_link` X's container chain by resolved path.
    let ancestors_for = |segs: &[String]| link_scope::link_ancestors(&nodes, &root_attrs, segs);
    let mut link_list = Vec::new();
    let mut datums = DatumTable::default();
    // `|datum|` nodes join the identity set first [SPEC 15.9] — one alphabet
    // per drawing scope, shared with the `>-` leader form below.
    collect_datum_nodes(
        &nodes,
        "",
        is_drawing(&root_attrs).then_some(""),
        &mut datums,
    )?;
    for w in &file.links {
        let (base, ancestors) = link_scope::link_scope(&baked, &root_attrs, &[]);
        let kind = link_scope_kind(&nodes, &root_attrs, &[]);
        collect_datum_letter(w, &[], &kind, &mut datums)?;
        let carried = resolve_carried(w, &ctx, &kind, &ancestors, &[], &root_text_ctx, &[])?;
        collect_datum_nodes(&carried, "", kind.drawing.then_some(""), &mut datums)?;
        link_list.extend(links::resolve_link(
            w,
            &ctx,
            &index,
            &[],
            &ancestors,
            &base,
            &kind,
            &ancestors_for,
            carried,
        )?);
    }
    for lw in &lifted {
        let (base, ancestors) = link_scope::link_scope(&baked, &root_attrs, &lw.chain);
        let kind = link_scope_kind(&nodes, &root_attrs, &lw.chain);
        collect_datum_letter(&lw.link, &lw.prefix, &kind, &mut datums)?;
        let carried = resolve_carried(
            &lw.link,
            &ctx,
            &kind,
            &ancestors,
            &lw.chain,
            &root_text_ctx,
            &lw.prefix,
        )?;
        let scope = lw.prefix.join(".");
        collect_datum_nodes(
            &carried,
            &scope,
            kind.drawing.then_some(scope.as_str()),
            &mut datums,
        )?;
        link_list.extend(links::resolve_link(
            &lw.link,
            &ctx,
            &index,
            &lw.prefix,
            &ancestors,
            &base,
            &kind,
            &ancestors_for,
            carried,
        )?);
    }

    let sheet_inputs = build_sheet_inputs(file, &vars, &funcs, &root_attrs, &baked, &sheet)?;

    Ok(Program {
        vars,
        scene: ResolvedScene {
            attrs: root_attrs,
            nodes,
        },
        links: link_list,
        sheet: sheet_inputs,
        // Carried to the layout phase for deferred `fn:` sampling [SPEC 14.3];
        // every borrow of it above (scene ctx, sheet inputs) has ended by here.
        funcs,
        datums: datums.letters,
    })
}

/// The per-scope datum identity set under construction [SPEC 15.7/15.9]:
/// `letters` in declaration order (surfaced on [`Program`] for the layout
/// phase's `datums:` validation), `seen` the duplicate gate across both
/// declaration forms — a letter is placed once, by `>-` or `|datum|`.
#[derive(Default)]
struct DatumTable {
    letters: HashMap<String, Vec<String>>,
    seen: HashMap<(String, String), crate::span::Span>,
}

impl DatumTable {
    fn place(&mut self, scope: &str, letter: &str, span: crate::span::Span) -> Result<(), Error> {
        let key = (scope.to_string(), letter.to_string());
        if let Some(prev) = self.seen.get(&key) {
            return Err(
                Error::at(span, format!("datum '{letter}' is already placed")).with_related(*prev),
            );
        }
        self.seen.insert(key, span);
        self.letters
            .entry(scope.to_string())
            .or_default()
            .push(letter.to_string());
        Ok(())
    }
}

/// Collect every `|datum|` node's letter [SPEC 15.9] — the framed letter as a
/// node, joining its drawing scope's identity set exactly as `>-` does.
/// `scope` is the nearest enclosing drawing's path (`None` outside one — the
/// node itself errors at layout, so nothing collects); an anonymous drawing
/// is path-transparent, mirroring the link prefix convention.
fn collect_datum_nodes(
    insts: &[ResolvedInst],
    path: &str,
    scope: Option<&str>,
    datums: &mut DatumTable,
) -> Result<(), Error> {
    for inst in insts {
        let child_path = match (&inst.id, path) {
            (None, _) => path.to_string(),
            (Some(id), "") => id.clone(),
            (Some(id), _) => format!("{path}.{id}"),
        };
        let child_scope = if is_drawing(&inst.attrs) {
            Some(child_path.as_str())
        } else {
            scope
        };
        if let Some(s) = child_scope
            && inst.type_chain.iter().any(|t| t == "datum")
            && let Some(letter) = inst
                .children
                .iter()
                .find(|c| c.kind == super::NodeKind::Text)
                .and_then(|c| c.label.as_deref())
        {
            datums.place(s, letter, inst.span)?;
        }
        collect_datum_nodes(&inst.children, &child_path, child_scope, datums)?;
    }
    Ok(())
}

/// Resolve a link statement's carried `[ ]` annotation nodes [SPEC 15.9]
/// through the ordinary node path — the same cascade, template classes, and
/// text inheritance a scene child gets, in the link's written scope. Only a
/// drawing's dimensions and leaders may carry nodes: outside a drawing scope a
/// node label errors ([SPEC 20]), and a carried node must be a drafting
/// annotation type.
fn resolve_carried(
    w: &crate::syntax::ast::Link,
    ctx: &SceneCtx,
    kind: &links::LinkScope,
    ancestors: &[NodeFacts],
    chain: &[scene::ScopeStep],
    root_text_ctx: &AttrMap,
    prefix: &[String],
) -> Result<Vec<ResolvedInst>, Error> {
    let mut out = Vec::new();
    for n in w.label_nodes() {
        if !kind.drawing {
            return Err(Error::at(
                n.span,
                "a routed link's '[ ]' holds text labels — annotation nodes ride a drawing's dimensions and leaders",
            ));
        }
        // The scope's inherited text context, rebuilt from the written chain —
        // exactly what a child of the innermost container would see.
        let mut text_ctx = root_text_ctx.clone();
        for step in chain {
            for name in properties::inherited_text() {
                if let Some(v) = step.attrs.get(name) {
                    text_ctx.insert(name, v.clone());
                }
            }
        }
        let mut walk = ancestors.to_vec();
        let mut steps = Vec::new();
        let mut ids = HashMap::new();
        let mut lifted = Vec::new();
        let inst = scene::resolve_node(
            n,
            ctx,
            &mut walk,
            &mut steps,
            prefix,
            &text_ctx,
            &mut ids,
            &mut lifted,
        )?;
        if !lifted.is_empty() {
            return Err(Error::at(n.span, "a carried annotation takes no links"));
        }
        if crate::glyph::drafting_type(&inst.type_chain).is_none() {
            return Err(Error::at(
                n.span,
                "a link's '[ ]' carries drafting annotations — '|surface-finish|', '|feature-control|', or '|datum|'",
            ));
        }
        out.push(inst);
    }
    Ok(out)
}

/// Collect a `>-` statement's datum letter [SPEC 15.7]: letters are
/// **identities**, gathered per drawing scope beside the id pass — a
/// duplicate errors with the first placement, and a feature-control frame's
/// `datums:` validates its references against the set at layout [SPEC 15.9].
/// The identity keys on the **operator** — a `marker: crow` restyles a wire,
/// never re-types it [SPEC 9] — and on the scope prefix: a drawing
/// statement's scope is always the drawing itself (the mate/measure gate
/// pins links to the immediate container), so sibling drawings each carry
/// their own alphabet.
fn collect_datum_letter(
    w: &crate::syntax::ast::Link,
    prefix: &[String],
    scope: &links::LinkScope,
    datums: &mut DatumTable,
) -> Result<(), Error> {
    use crate::ast::{ChainOp, LinkMarker};
    let datum_leader = matches!(w.op(), ChainOp::Wire(op)
        if op.start == LinkMarker::Crow && op.end == LinkMarker::None);
    if !scope.drawing || !datum_leader || w.chain.len() != 1 {
        return Ok(());
    }
    let Some(letter) = w.label_texts().next() else {
        return Ok(()); // the empty-leader gate reports this one
    };
    datums.place(&prefix.join("."), &letter.text, letter.span)
}

// ─────────────────────────── Root config ───────────────────────────

/// Root container attributes — read straight from the global block. Desugar
/// injects the scene defaults (`layout: flow`, `padding: 20` — the scene's
/// frame — `gap`, the inherited-text baseline), so there is nothing to seed here.
fn root_attrs(file: &File, vars: &VarTable, funcs: &FuncTable) -> Result<AttrMap, Error> {
    let mut ordered: Vec<(String, ResolvedValue)> = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::RootDecl(d) = item {
            ordered.push((
                d.name.clone(),
                resolve_property(&d.name, &d.groups, d.span, vars, funcs)?,
            ));
        }
    }
    Ok(collapse(&ordered))
}

/// The renderer's [`SheetInputs`]: every single-class rule's attrs (the generated
/// `.lini-*` type classes and the user `.style` classes, in source order), the
/// link defaults, and the root inherited-text font size. Descendant rules
/// (`|.lini-table .lini-box| { }`) bake inline via the cascade and carry no entry.
fn build_sheet_inputs(
    file: &File,
    vars: &VarTable,
    funcs: &FuncTable,
    root_attrs: &AttrMap,
    baked: &[(String, ResolvedValue)],
    sheet: &Stylesheet,
) -> Result<SheetInputs, Error> {
    let mut class_rules = Vec::new();
    let mut descendant_rules = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item {
            match r.selector.units.as_slice() {
                [SelUnit::Class(c)] => {
                    class_rules.push((c.clone(), decls_attrmap(&r.decls, vars, funcs)?));
                }
                [SelUnit::Class(a), SelUnit::Class(b)] => {
                    descendant_rules.push((
                        a.clone(),
                        b.clone(),
                        decls_attrmap(&r.decls, vars, funcs)?,
                    ));
                }
                _ => {}
            }
        }
    }
    // The `.lini-link` rule's defaults: a root-scope link — the baked base plus the
    // scope config, then the root `|-|` element rule [SPEC 9, 16]. Its paint states
    // the `.lini-link` CSS rule; a link that differs inlines the difference.
    let (base, _) = link_scope::link_scope(baked, root_attrs, &[]);
    let mut link_defaults = base;
    link_defaults.extend(sheet.class_decls(links::LINK_CLASS));
    let link_defaults = collapse(&link_defaults);
    let root_font_size = root_attrs
        .number("font-size")
        .unwrap_or(consts::ROOT_FONT_SIZE);
    // Inherited-text props the global block set, for the `.lini` rule [SPEC 6].
    // `font-family` / `font-weight` / `color` override their themeable var when set
    // globally; the rest are live CSS with no default, present only when authored.
    // The baked-spacing props are layout (`font-size` is the baked root literal),
    // so the live-CSS subset is the inherited text set minus the baked one.
    let mut root_text = AttrMap::new();
    for name in properties::inherited_text().filter(|n| !properties::is_baked_text(n)) {
        if let Some(v) = root_attrs.get(name) {
            root_text.insert(name, v.clone());
        }
    }
    Ok(SheetInputs {
        class_rules,
        descendant_rules,
        link_defaults,
        root_font_size,
        root_text,
    })
}

fn decls_attrmap(decls: &[Decl], vars: &VarTable, funcs: &FuncTable) -> Result<AttrMap, Error> {
    let mut ordered = Vec::with_capacity(decls.len());
    for d in decls {
        ordered.push((
            d.name.clone(),
            resolve_property(&d.name, &d.groups, d.span, vars, funcs)?,
        ));
    }
    Ok(collapse(&ordered))
}
