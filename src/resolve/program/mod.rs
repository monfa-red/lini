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
    let mut link_list = Vec::new();
    for w in &file.links {
        let (base, ancestors) = link_scope::link_scope(&baked, &nodes, &root_attrs, &[]);
        let kind = link_scope_kind(&nodes, &root_attrs, &[]);
        link_list.extend(links::resolve_link(
            w,
            &ctx,
            &index,
            &[],
            &ancestors,
            &base,
            &kind,
        )?);
    }
    for lw in &lifted {
        let (base, ancestors) = link_scope::link_scope(&baked, &nodes, &root_attrs, &lw.prefix);
        let kind = link_scope_kind(&nodes, &root_attrs, &lw.prefix);
        link_list.extend(links::resolve_link(
            &lw.link, &ctx, &index, &lw.prefix, &ancestors, &base, &kind,
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
    })
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
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item
            && let [SelUnit::Class(c)] = r.selector.units.as_slice()
        {
            class_rules.push((c.clone(), decls_attrmap(&r.decls, vars, funcs)?));
        }
    }
    // The `.lini-link` rule's defaults: a root-scope link — the baked base plus the
    // scope config, then the root `|-|` element rule [SPEC 9, 16]. Its paint states
    // the `.lini-link` CSS rule; a link that differs inlines the difference.
    let (base, _) = link_scope::link_scope(baked, &[], root_attrs, &[]);
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
