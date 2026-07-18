//! GD&T frames [SPEC 15.9] — `|feature-control|` with its `|control|` rows,
//! and the `|datum|` node. A frame is boxed compartments at font-derived
//! sizes: the characteristic glyph, the zone-prefixed tolerance with its
//! material and ordered modifiers, then the datum references — every cell
//! bordered at annotation-linework weight. The ISO 1101 validity table is
//! enforced here: a frame renders semantically valid or errors with a
//! correction, never plausible-looking and wrong ([SPEC 20]).

use super::super::ir::{Bbox, PlacedNode};
use super::super::{approx_width, prim};
use super::symbols::{self, SymbolPaint};
use crate::error::Error;
use crate::glyph::GRID;
use crate::resolve::{NodeKind, Program, ResolvedInst, ResolvedValue};

/// Whether a characteristic takes datum references [SPEC 15.9].
#[derive(PartialEq, Clone, Copy)]
enum DatumRule {
    Forbidden,
    Optional,
    Required,
}

/// One ISO 1101 characteristic: its geometric group (worded into the
/// form-control error), its datum rule, and whether its zone is axial
/// (`zone:` legal) / it controls a feature of size (`material:` legal).
struct Characteristic {
    name: &'static str,
    group: &'static str,
    datums: DatumRule,
    axial: bool,
    feature_of_size: bool,
}

/// The fourteen [SPEC 15.9] — ISO 1101's canonical set (ASME dropped
/// `concentricity` / `symmetry`; lini's drafting lineage is ISO).
#[rustfmt::skip]
const CHARACTERISTICS: &[Characteristic] = &[
    ch("straightness",     "form",        DatumRule::Forbidden, true,  true),
    ch("flatness",         "form",        DatumRule::Forbidden, false, false),
    ch("circularity",      "form",        DatumRule::Forbidden, false, false),
    ch("cylindricity",     "form",        DatumRule::Forbidden, false, false),
    ch("profile-line",     "profile",     DatumRule::Optional,  false, false),
    ch("profile-surface",  "profile",     DatumRule::Optional,  false, false),
    ch("angularity",       "orientation", DatumRule::Required,  true,  true),
    ch("perpendicularity", "orientation", DatumRule::Required,  true,  true),
    ch("parallelism",      "orientation", DatumRule::Required,  true,  true),
    ch("position",         "location",    DatumRule::Optional,  true,  true),
    ch("concentricity",    "location",    DatumRule::Required,  true,  false),
    ch("symmetry",         "location",    DatumRule::Required,  false, false),
    ch("circular-runout",  "runout",      DatumRule::Required,  false, false),
    ch("total-runout",     "runout",      DatumRule::Required,  false, false),
];

const fn ch(
    name: &'static str,
    group: &'static str,
    datums: DatumRule,
    axial: bool,
    feature_of_size: bool,
) -> Characteristic {
    Characteristic {
        name,
        group,
        datums,
        axial,
        feature_of_size,
    }
}

fn characteristic(name: &str) -> Option<&'static Characteristic> {
    CHARACTERISTICS.iter().find(|c| c.name == name)
}

/// One validated control row [SPEC 15.9] — everything the compartments
/// draw, in compartment order.
struct Row {
    ch: &'static Characteristic,
    /// The tolerance reading: the ⌀ / S⌀ zone prefix + the zone width.
    tol: String,
    /// The Ⓜ / Ⓛ after the value, as its glyph name.
    material: Option<&'static str>,
    /// The ordered extras after the material modifier.
    modifiers: Vec<Modifier>,
    /// Primary → tertiary references, each with an optional modifier glyph.
    datums: Vec<(String, Option<&'static str>)>,
}

enum Modifier {
    Projected(f64),
    FreeState,
    TangentPlane,
}

/// A property's comma-groups, normalized: each group a slice of scalars.
fn value_groups(v: &ResolvedValue) -> Vec<Vec<&ResolvedValue>> {
    match v {
        ResolvedValue::List(items) => items
            .iter()
            .map(|it| match it {
                ResolvedValue::Tuple(xs) => xs.iter().collect(),
                one => vec![one],
            })
            .collect(),
        ResolvedValue::Tuple(xs) => vec![xs.iter().collect()],
        one => vec![vec![one]],
    }
}

fn as_ident(v: &ResolvedValue) -> Option<&str> {
    match v {
        ResolvedValue::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

/// The row's smart label — its characteristic [SPEC 15.9].
fn label_of(inst: &ResolvedInst) -> Option<&str> {
    inst.children
        .iter()
        .find(|c| c.kind == NodeKind::Text)
        .and_then(|c| c.label.as_deref())
}

/// Parse and validate one control row — a `|control|`, or the frame itself
/// in one-row form — against the characteristic validity table [SPEC 15.9];
/// `declared` is the scope's collected datum alphabet.
fn parse_row(inst: &ResolvedInst, declared: &[String]) -> Result<Row, Error> {
    let err = |msg: String| Err(Error::at(inst.span, msg));

    // The characteristic: the smart label or `characteristic:` [SPEC 15.9].
    let label = label_of(inst);
    let longhand = inst.attrs.get("characteristic").and_then(as_ident);
    let name = match (label, longhand) {
        (Some(_), Some(_)) => {
            return err(
                "a control's characteristic is its label or 'characteristic:', not both".into(),
            );
        }
        (Some(l), None) => l,
        (None, Some(c)) => c,
        (None, None) => {
            return err(
                "a control names its characteristic — its label or 'characteristic:'".into(),
            );
        }
    };
    let Some(ch) = characteristic(name) else {
        let near = crate::suggest::nearest(name, CHARACTERISTICS.iter().map(|c| c.name), 3);
        return err(format!(
            "unknown characteristic '{name}'{}",
            crate::suggest::did_you_mean(&near)
        ));
    };

    // `tol:` — the zone width, required and positive [SPEC 15.9].
    let tol = match inst.attrs.number("tol") {
        Some(t) if t > 0.0 => t,
        _ => return err("a control row needs 'tol' — its zone width".into()),
    };

    // `zone:` — the ⌀ / S⌀ prefix, axial-zone characteristics only.
    let zone = match inst.attrs.get("zone").map(|v| (v, as_ident(v))) {
        None => "",
        Some((_, Some(z @ ("diameter" | "spherical")))) => {
            if !ch.axial {
                return err(format!(
                    "'zone: {z}' has no meaning on '{}' — its zone is a width, not an axis",
                    ch.name
                ));
            }
            if z == "diameter" { "⌀" } else { "S⌀" }
        }
        Some(_) => return err("'zone' takes 'diameter' or 'spherical'".into()),
    };

    // `material:` — Ⓜ / Ⓛ, feature-of-size controls only.
    let material = match inst.attrs.get("material").map(as_ident) {
        None => None,
        Some(Some(m @ ("maximum" | "least"))) => {
            if !ch.feature_of_size {
                return err(
                    "'material' modifies a feature-of-size control — position, orientation, or straightness"
                        .into(),
                );
            }
            Some(material_glyph(m))
        }
        Some(_) => return err("'material' takes 'maximum' or 'least'".into()),
    };

    // `modifiers:` — the ordered extras [SPEC 15.9].
    let mut modifiers = Vec::new();
    if let Some(v) = inst.attrs.get("modifiers") {
        for group in value_groups(v) {
            let m = match group.as_slice() {
                [one] => match as_ident(one) {
                    Some("free-state") => Some(Modifier::FreeState),
                    Some("tangent-plane") => Some(Modifier::TangentPlane),
                    _ => None,
                },
                [kind, ResolvedValue::Number(n)] if *n > 0.0 => match as_ident(kind) {
                    Some("projected") => Some(Modifier::Projected(*n)),
                    _ => None,
                },
                _ => None,
            };
            match m {
                Some(m) => modifiers.push(m),
                None => {
                    return err(
                        "'modifiers' takes projected N, free-state, or tangent-plane".into(),
                    );
                }
            }
        }
    }

    // `datums:` — primary → tertiary, each declared in the scope [SPEC 15.9].
    let mut datums = Vec::new();
    if let Some(v) = inst.attrs.get("datums") {
        for group in value_groups(v) {
            let d = match group.as_slice() {
                [letter] => as_ident(letter).map(|l| (l.to_string(), None)),
                [letter, m] => match (as_ident(letter), as_ident(m)) {
                    (Some(l), Some(m @ ("maximum" | "least"))) => {
                        Some((l.to_string(), Some(material_glyph(m))))
                    }
                    _ => None,
                },
                _ => None,
            };
            match d {
                Some(d) => datums.push(d),
                None => {
                    return err(
                        "'datums' takes letters, each with an optional 'maximum' / 'least'".into(),
                    );
                }
            }
        }
    }
    if datums.len() > 3 {
        return err("'datums' orders primary, secondary, tertiary — three at most".into());
    }
    match ch.datums {
        DatumRule::Forbidden if !datums.is_empty() => {
            return err(format!(
                "'{}' is a {} control — it takes no datum",
                ch.name, ch.group
            ));
        }
        DatumRule::Required if datums.is_empty() => {
            return err(format!(
                "'{}' measures against a datum — name one in 'datums:'",
                ch.name
            ));
        }
        _ => {}
    }
    for (letter, _) in &datums {
        if !declared.iter().any(|d| d == letter) {
            let set = if declared.is_empty() {
                "none declared".to_string()
            } else {
                format!("declared: {}", declared.join(", "))
            };
            return err(format!("no datum '{letter}' in this drawing — {set}"));
        }
    }

    Ok(Row {
        ch,
        tol: format!("{zone}{}", super::compose::fmt(tol)),
        material,
        modifiers,
        datums,
    })
}

fn material_glyph(m: &str) -> &'static str {
    if m == "maximum" {
        "modifier-maximum"
    } else {
        "modifier-least"
    }
}

/// The datum alphabet of the drawing scope enclosing `path` — collected at
/// resolve ([SPEC 15.7/15.9]; `""` covers the root scope and the
/// path-transparent anonymous drawing).
fn scope_letters<'a>(path: &str, program: &'a Program) -> &'a [String] {
    let mut scope = "";
    for (i, _) in path.match_indices('.') {
        if super::is_drawing_scope(program, &path[..i]) {
            scope = &path[..i];
        }
    }
    // An anonymous frame's own path is its parent's — it may *be* the scope.
    if super::is_drawing_scope(program, path) {
        scope = path;
    }
    program.datums.get(scope).map(Vec::as_slice).unwrap_or(&[])
}

// ── Compartment geometry [SPEC 15.9] — all font-derived ──

/// A cell's inner inset each side (the datum box's 6-unit total, [SPEC 15.7]).
const PAD: f64 = 3.0;
/// The gap between items sharing a compartment (a value and its Ⓜ).
const GAP: f64 = 2.0;

/// One drawable item inside a compartment.
enum Item {
    Text(String),
    Glyph(&'static str),
}

impl Item {
    /// Natural-units width [SPEC 15.9]: a glyph at height `fs` off its grid
    /// width, text at the annotation font.
    fn width(&self, font: crate::font::Font, fs: f64) -> f64 {
        match self {
            Item::Text(t) => approx_width(t, font, fs, 0.0),
            Item::Glyph(name) => {
                crate::glyph::lookup(name)
                    .expect("a registry modifier")
                    .width
                    * fs
                    / GRID
            }
        }
    }
}

/// The row's compartments past the (frame-shared) symbol cell: the tolerance
/// reading with its modifiers, then one cell per datum reference.
fn row_cells(row: &Row) -> Vec<Vec<Item>> {
    let mut tol = vec![Item::Text(row.tol.clone())];
    if let Some(m) = row.material {
        tol.push(Item::Glyph(m));
    }
    for m in &row.modifiers {
        match m {
            Modifier::Projected(n) => {
                tol.push(Item::Glyph("modifier-projected"));
                tol.push(Item::Text(super::compose::fmt(*n)));
            }
            Modifier::FreeState => tol.push(Item::Glyph("modifier-free-state")),
            Modifier::TangentPlane => tol.push(Item::Glyph("modifier-tangent-plane")),
        }
    }
    let mut cells = vec![tol];
    for (letter, m) in &row.datums {
        let mut cell = vec![Item::Text(letter.clone())];
        if let Some(m) = *m {
            cell.push(Item::Glyph(m));
        }
        cells.push(cell);
    }
    cells
}

fn cell_width(items: &[Item], font: crate::font::Font, fs: f64, floor: f64) -> f64 {
    let inner: f64 = items.iter().map(|i| i.width(font, fs)).sum::<f64>()
        + GAP * (items.len().saturating_sub(1)) as f64;
    (inner + 2.0 * PAD).max(floor)
}

/// One bordered compartment: the `--bg`-backed cell rect at annotation
/// linework, then its items laid left-to-right, vertically centred.
#[allow(clippy::too_many_arguments)]
fn lower_cell(
    out: &mut Vec<PlacedNode>,
    items: &[Item],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    paint: &SymbolPaint,
    fill: &ResolvedValue,
    font: crate::font::Font,
) {
    let mut rect = prim::rect(x + w / 2.0, y + h / 2.0, w, h, fill.clone(), 1.0);
    prim::outline(&mut rect, paint.stroke.clone(), paint.sw);
    out.push(rect);
    // Items centre as a group in their cell (a lone value reads centred, a
    // modifier run stays inside the borders).
    let total: f64 = items.iter().map(|i| i.width(font, paint.fs)).sum::<f64>()
        + GAP * (items.len().saturating_sub(1)) as f64;
    let mut ix = x + (w - total) / 2.0;
    let cy = y + h / 2.0;
    for item in items {
        let iw = item.width(font, paint.fs);
        match item {
            Item::Text(t) => out.push(prim::dim_text(t, ix + iw / 2.0, cy, paint.fs, font.kind)),
            Item::Glyph(name) => out.push(prim::glyph(
                name,
                ix + iw / 2.0,
                cy,
                iw,
                paint.fs,
                paint.stroke.clone(),
                paint.sw,
            )),
        }
        ix += iw + GAP;
    }
}

/// Lower a `|feature-control|` frame [SPEC 15.9]: validate its rows (the
/// one-row sugar or `|control|` children — mixing errors), then draw the
/// boxed compartments — adjacent rows sharing a characteristic merge its
/// symbol cell (the composite frame), differing rows stack combined, in
/// source order.
pub(in crate::layout) fn layout_frame(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    let declared = scope_letters(path, program);
    let controls: Vec<&ResolvedInst> = inst
        .children
        .iter()
        .filter(|c| c.type_chain.iter().any(|t| t == "control"))
        .collect();
    let own_row = label_of(inst).is_some()
        || [
            "characteristic",
            "tol",
            "zone",
            "material",
            "datums",
            "modifiers",
        ]
        .iter()
        .any(|p| inst.attrs.get(p).is_some());
    let rows: Vec<Row> = if controls.is_empty() {
        vec![parse_row(inst, declared)?]
    } else {
        if own_row {
            return Err(Error::at(
                inst.span,
                "a frame is one row or '|control|' rows — not both",
            ));
        }
        controls
            .iter()
            .map(|c| parse_row(c, declared))
            .collect::<Result<_, _>>()?
    };

    let paint = SymbolPaint::of(inst);
    let font = inst.font;
    let fill = inst
        .attrs
        .get("fill")
        .cloned()
        .unwrap_or(ResolvedValue::LiveVar {
            name: "bg".into(),
            raw: false,
        });
    // The frame anatomy shares the datum box's height [SPEC 15.7/15.9]; the
    // symbol compartment is square at it (the characteristic glyphs are
    // grid-square).
    let h = paint.fs + 6.0;
    let sym_w = h;
    let cells: Vec<Vec<Vec<Item>>> = rows.iter().map(row_cells).collect();
    let widths: Vec<Vec<f64>> = cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|c| cell_width(c, font, paint.fs, h))
                .collect()
        })
        .collect();
    let total_w = sym_w
        + widths
            .iter()
            .map(|w| w.iter().sum::<f64>())
            .fold(0.0_f64, f64::max);
    let total_h = h * rows.len() as f64;
    let (x0, y0) = (-total_w / 2.0, -total_h / 2.0);

    let mut children = Vec::new();
    // The symbol column: consecutive rows with one characteristic share a
    // merged compartment — one cell, one glyph, spanning the run.
    let mut i = 0;
    while i < rows.len() {
        let mut j = i + 1;
        while j < rows.len() && std::ptr::eq(rows[j].ch, rows[i].ch) {
            j += 1;
        }
        let span_h = h * (j - i) as f64;
        let y = y0 + h * i as f64;
        let mut rect = prim::rect(
            x0 + sym_w / 2.0,
            y + span_h / 2.0,
            sym_w,
            span_h,
            fill.clone(),
            1.0,
        );
        prim::outline(&mut rect, paint.stroke.clone(), paint.sw);
        children.push(rect);
        let g = crate::glyph::lookup(rows[i].ch.name).expect("a validated characteristic");
        let gw = g.width * paint.fs / GRID;
        children.push(prim::glyph(
            rows[i].ch.name,
            x0 + sym_w / 2.0,
            y + span_h / 2.0,
            gw,
            paint.fs,
            paint.stroke.clone(),
            paint.sw,
        ));
        i = j;
    }
    // The rows' remaining compartments, left-aligned after the symbol column.
    for (r, row) in cells.iter().enumerate() {
        let y = y0 + h * r as f64;
        let mut x = x0 + sym_w;
        for (c, items) in row.iter().enumerate() {
            let w = widths[r][c];
            lower_cell(&mut children, items, x, y, w, h, &paint, &fill, font);
            x += w;
        }
    }

    let bbox = Bbox {
        min_x: x0,
        min_y: y0,
        max_x: x0 + total_w,
        max_y: y0 + total_h,
    };
    Ok(symbols::shell(inst, bbox, children))
}

/// Lower a `|datum|` node [SPEC 15.9]: the framed letter, one anatomy with
/// the `>-` leader's box (`symbols::framed_letter_size` / `datum_frame_box`),
/// backed `--bg` and centred on the node's origin.
pub(in crate::layout) fn layout_datum(inst: &ResolvedInst) -> Result<PlacedNode, Error> {
    let Some(letter) = label_of(inst) else {
        return Err(Error::at(
            inst.span,
            "a '|datum|' names its letter — '|datum| \"A\"'",
        ));
    };
    let paint = SymbolPaint::of(inst);
    let font = inst.font;
    let (w, h) = symbols::framed_letter_size(letter, font, paint.fs);
    let fill = inst
        .attrs
        .get("fill")
        .cloned()
        .unwrap_or(ResolvedValue::LiveVar {
            name: "bg".into(),
            raw: false,
        });
    let children = vec![
        prim::rect(0.0, 0.0, w, h, fill, 1.0),
        symbols::datum_frame_box((0.0, 0.0), w, h, paint.stroke.clone(), paint.sw),
        prim::dim_text(letter, 0.0, 0.0, paint.fs, font.kind),
    ];
    Ok(symbols::shell(inst, Bbox::centered(w, h), children))
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, texts};
    use crate::layout::PlacedNode;
    use crate::resolve::{NodeKind, ResolvedValue};

    const PART: &str = "{ layout: drawing; density: 1 }\n|rect#a| { width: 80; height: 40 }\na:bottom >- \"A\"\na:right >- \"B\"\n";

    fn compile_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        match crate::desugar::desugar(&file)
            .and_then(|low| crate::resolve::resolve_with_theme(&low, &[]).map(|_| ()))
        {
            Ok(()) => panic!("expected a resolve error"),
            Err(e) => e.message,
        }
    }

    /// Every drafting glyph under `n`, as its registry symbol name.
    fn glyphs(n: &PlacedNode) -> Vec<String> {
        fn walk(n: &PlacedNode, out: &mut Vec<String>) {
            if n.type_chain.iter().any(|t| t == "drafting-glyph")
                && let Some(ResolvedValue::Ident(s)) = n.attrs.get("symbol")
            {
                out.push(s.clone());
            }
            n.children.iter().for_each(|c| walk(c, out));
        }
        let mut out = Vec::new();
        walk(n, &mut out);
        out
    }

    #[test]
    fn a_single_row_frame_lowers_its_compartments() {
        let out = laid(&format!(
            "{PART}|feature-control#fcf| \"position\" {{ tol: 0.1; zone: diameter; material: maximum; datums: A, B least; translate: 0 -60 }}\n"
        ));
        let fcf = by_id(&out.nodes, "fcf");
        // Symbol + Ⓜ in the tol cell + Ⓛ on the secondary datum.
        assert_eq!(
            glyphs(fcf),
            ["position", "modifier-maximum", "modifier-least"]
        );
        // The zone-prefixed tolerance and the two datum letters.
        let labels: Vec<&str> = fcf
            .children
            .iter()
            .filter(|c| c.kind == NodeKind::Text)
            .filter_map(|c| c.label.as_deref())
            .collect();
        assert_eq!(labels, ["⌀0.1", "A", "B"]);
        // Compartments at font-derived sizes [SPEC 15.9]: cells are fs+6 tall,
        // the glyphs fs tall, borders at the statement's stroke width.
        assert!((fcf.bbox.h() - 18.0).abs() < 1e-9);
        let cells: Vec<&PlacedNode> = fcf
            .children
            .iter()
            .filter(|c| c.kind == NodeKind::Block)
            .collect();
        assert_eq!(cells.len(), 4); // symbol, tol, datum, datum
        for cell in cells {
            assert!((cell.bbox.h() - 18.0).abs() < 1e-9);
            assert_eq!(cell.attrs.number("stroke-width"), Some(1.0));
        }
        let glyph = fcf
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Icon)
            .unwrap();
        assert!((glyph.bbox.h() - 12.0).abs() < 1e-9);
        assert_eq!(glyph.attrs.number("stroke-width"), Some(1.0));
    }

    #[test]
    fn longhand_characteristic_equals_the_label() {
        let a = laid(&format!(
            "{PART}|feature-control#f| \"flatness\" {{ tol: 0.05; translate: 0 -60 }}\n"
        ));
        let b = laid(&format!(
            "{PART}|feature-control#f| {{ characteristic: flatness; tol: 0.05; translate: 0 -60 }}\n"
        ));
        assert_eq!(glyphs(by_id(&a.nodes, "f")), glyphs(by_id(&b.nodes, "f")));
    }

    #[test]
    fn adjacent_rows_sharing_a_characteristic_merge_its_symbol_cell() {
        let out = laid(&format!(
            "{PART}|feature-control#comp| {{ translate: 0 -70 }} [\n|control| \"position\" {{ tol: 0.4; zone: diameter; datums: A }}\n|control| \"position\" {{ tol: 0.1; zone: diameter; datums: A }}\n|control| \"flatness\" {{ tol: 0.05 }}\n]\n"
        ));
        let comp = by_id(&out.nodes, "comp");
        // One merged position glyph spanning two rows, then flatness's own.
        assert_eq!(glyphs(comp), ["position", "flatness"]);
        assert!((comp.bbox.h() - 3.0 * 18.0).abs() < 1e-9);
        // The merged symbol cell is two rows tall.
        let sym_cell = comp
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Block)
            .unwrap();
        assert!((sym_cell.bbox.h() - 36.0).abs() < 1e-9);
    }

    #[test]
    fn a_view_scale_never_touches_a_frame() {
        // Sheet content [SPEC 15.1/15.9]: geometry doubles, the frame holds.
        let at = |scale: &str| {
            let out = laid(&format!(
                "{{ layout: drawing; density: 1 }}\n|rect#a| {{ width: 80; height: 40; {scale} }}\na:bottom >- \"A\"\n|feature-control#f| \"position\" {{ tol: 0.1; datums: A; translate: 0 -60 }}\n"
            ));
            by_id(&out.nodes, "f").bbox.h()
        };
        assert_eq!(at(""), at("scale: 2"));
    }

    #[test]
    fn a_dim_row_packs_past_a_placed_frame() {
        // The frame sits where the bottom row would seat [SPEC 15.6/15.9]:
        // the row stands clear below it, never overlapping.
        let out = laid(&format!(
            "{PART}|feature-control#f| \"position\" {{ tol: 0.1; datums: A; translate: 0 28 }}\na:left (-) a:right {{ side: bottom }}\n"
        ));
        let f = by_id(&out.nodes, "f");
        let f_box = f.bbox.shifted(f.cx, f.cy);
        let (_, vy, _) = super::super::testutil::text_at(&out.nodes, "80");
        assert!(
            vy > f_box.max_y,
            "dim value at y {vy} inside the frame (bottom {})",
            f_box.max_y
        );
    }

    #[test]
    fn the_leader_form_wires_the_one_placed_frame() {
        let out = laid(&format!(
            "{PART}|feature-control#f| \"position\" {{ tol: 0.1; datums: A; translate: 0 -70 }}\na:top <- f\n"
        ));
        assert_eq!(glyphs(by_id(&out.nodes, "f")), ["position"]);
        assert_eq!(
            texts(&out.nodes).iter().filter(|(t, ..)| t == "⌀").count(),
            0
        );
    }

    #[test]
    fn a_datum_node_shares_the_leader_boxs_anatomy() {
        let out = laid(&format!("{PART}|datum#c| \"C\" {{ translate: 0 -60 }}\n"));
        let c = by_id(&out.nodes, "c");
        let frame = c
            .children
            .iter()
            .find(|n| n.type_chain.iter().any(|t| t == "datum-frame"))
            .expect("the framed letter");
        // The `>-` box anatomy [SPEC 15.7]: fs+6 square, dim-line classed.
        assert!((frame.bbox.h() - 18.0).abs() < 1e-9);
        assert!((frame.bbox.w() - 18.0).abs() < 1e-9);
        assert!(frame.type_chain.iter().any(|t| t == "dim-line"));
        assert_eq!(
            super::super::testutil::text_at(&out.nodes, "C").1,
            c.cy // the letter centres in the node
        );
    }

    #[test]
    fn a_datum_nodes_letter_joins_the_scope_alphabet() {
        // Referencing the node-declared letter validates [SPEC 15.9].
        let out = laid(&format!(
            "{PART}|datum#c| \"C\" {{ translate: 0 -60 }}\n|feature-control#f| \"position\" {{ tol: 0.1; datums: C; translate: 0 -90 }}\n"
        ));
        assert_eq!(glyphs(by_id(&out.nodes, "f")), ["position"]);
    }

    // ── The SPEC 20 validity rows — a frame is semantically valid or an
    //    error, never plausible-wrong. ──

    #[test]
    fn an_unknown_characteristic_suggests_the_nearest() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"flatnes\" {{ tol: 0.1 }}\n"
            )),
            "unknown characteristic 'flatnes'; did you mean 'flatness'?"
        );
    }

    #[test]
    fn a_characteristic_set_twice_errors() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"flatness\" {{ characteristic: flatness; tol: 0.1 }}\n"
            )),
            "a control's characteristic is its label or 'characteristic:', not both"
        );
    }

    #[test]
    fn a_missing_characteristic_errors() {
        assert_eq!(
            layout_err(&format!("{PART}|feature-control| {{ tol: 0.1 }}\n")),
            "a control names its characteristic — its label or 'characteristic:'"
        );
    }

    #[test]
    fn a_row_without_tol_errors_and_zero_is_no_zone() {
        let msg = "a control row needs 'tol' — its zone width";
        assert_eq!(
            layout_err(&format!("{PART}|feature-control| \"flatness\"\n")),
            msg
        );
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"flatness\" {{ tol: 0 }}\n"
            )),
            msg
        );
    }

    #[test]
    fn mixing_the_one_row_and_control_forms_errors() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1 }} [\n|control| \"flatness\" {{ tol: 0.05 }}\n]\n"
            )),
            "a frame is one row or '|control|' rows — not both"
        );
    }

    #[test]
    fn a_control_outside_a_frame_errors() {
        assert_eq!(
            layout_err(&format!("{PART}|control| \"flatness\" {{ tol: 0.05 }}\n")),
            "'|control|' is a '|feature-control|' row"
        );
    }

    #[test]
    fn the_frame_types_error_outside_a_drawing() {
        for ty in ["feature-control", "control", "datum"] {
            assert_eq!(
                layout_err(&format!("|{ty}| \"A\"\n|box#a|\n")),
                format!("'|{ty}|' annotates a drawing — it belongs in a 'layout: drawing'")
            );
        }
    }

    #[test]
    fn datums_on_a_form_control_error() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"flatness\" {{ tol: 0.1; datums: A }}\n"
            )),
            "'flatness' is a form control — it takes no datum"
        );
    }

    #[test]
    fn a_missing_required_datum_errors() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"circular-runout\" {{ tol: 0.05 }}\n"
            )),
            "'circular-runout' measures against a datum — name one in 'datums:'"
        );
    }

    #[test]
    fn an_unknown_datum_reference_names_the_declared_set() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; datums: D }}\n"
            )),
            "no datum 'D' in this drawing — declared: A, B"
        );
        // With no letters declared at all, the message says so.
        assert_eq!(
            layout_err(
                "{ layout: drawing; density: 1 }\n|rect#a| { width: 80; height: 40 }\n|feature-control| \"position\" { tol: 0.1; datums: D }\n"
            ),
            "no datum 'D' in this drawing — none declared"
        );
    }

    #[test]
    fn more_than_three_datums_error() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; datums: A, B, A, B }}\n"
            )),
            "'datums' orders primary, secondary, tertiary — three at most"
        );
    }

    #[test]
    fn a_bad_per_datum_modifier_errors() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; datums: A tangent }}\n"
            )),
            "'datums' takes letters, each with an optional 'maximum' / 'least'"
        );
    }

    #[test]
    fn zone_off_an_axial_control_errors() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"flatness\" {{ tol: 0.1; zone: diameter }}\n"
            )),
            "'zone: diameter' has no meaning on 'flatness' — its zone is a width, not an axis"
        );
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; zone: round; datums: A }}\n"
            )),
            "'zone' takes 'diameter' or 'spherical'"
        );
    }

    #[test]
    fn material_off_a_feature_of_size_control_errors() {
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"circularity\" {{ tol: 0.1; material: maximum }}\n"
            )),
            "'material' modifies a feature-of-size control — position, orientation, or straightness"
        );
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; material: max; datums: A }}\n"
            )),
            "'material' takes 'maximum' or 'least'"
        );
    }

    #[test]
    fn an_unknown_modifier_errors() {
        let msg = "'modifiers' takes projected N, free-state, or tangent-plane";
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; datums: A; modifiers: shiny }}\n"
            )),
            msg
        );
        // `projected` without its length is as unknown.
        assert_eq!(
            layout_err(&format!(
                "{PART}|feature-control| \"position\" {{ tol: 0.1; datums: A; modifiers: projected }}\n"
            )),
            msg
        );
    }

    #[test]
    fn a_letterless_datum_node_errors() {
        assert_eq!(
            layout_err(&format!("{PART}|datum#c| {{ translate: 0 -60 }}\n")),
            "a '|datum|' names its letter — '|datum| \"A\"'"
        );
    }

    #[test]
    fn a_duplicate_letter_across_both_forms_errors_at_resolve() {
        // `>-` placed A first; the `|datum|` node collides [SPEC 15.7/15.9].
        assert_eq!(
            compile_err(&format!("{PART}|datum#dup| \"A\" {{ translate: 0 -60 }}\n")),
            "datum 'A' is already placed"
        );
        // …and two nodes collide with each other.
        assert_eq!(
            compile_err(
                "{ layout: drawing; density: 1 }\n|rect#a| { width: 80; height: 40 }\n|datum#one| \"C\"\n|datum#two| \"C\"\n"
            ),
            "datum 'C' is already placed"
        );
    }
}
