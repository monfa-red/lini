//! **The out-of-scope type gate** [SPEC 21] — one sweep of the resolved tree,
//! before anything places, that says where a layout's own types may be written:
//! a schematic type in a `layout: schematic`, a chart series / `|axis|` /
//! `|band|` / `|mark|` in a `layout: chart`, a `|slice|` in a `layout: pie`.
//! One walk, one law per family, so a new family is a row here and never a
//! second traversal ([`crate::layout::chart::out_of_scope`] is the chart's
//! reading; [`crate::desugar::schematic::schematic_type`] the schematic's).
//!
//! The scope is **carried down the walk**, not read back off a dot-path: an
//! anonymous container contributes no path segment [SPEC 9], so an anonymous
//! `|schematic|` — or an anonymous part inside one — is invisible to a path
//! predicate and plain to this. Desugar carries the same law the same way
//! ([`crate::desugar::Nest`]), which is what makes the two stages agree.
//!
//! Placement still does not cascade — a nested `|row|` runs its own engine —
//! but the *laws* reach it, so `|R|` inside a row inside a sheet is legal.
//!
//! **This walk is not sealed, and the statement laws are** — deliberately.
//! A nested `|sequence|` or `|drawing|` stops the *reading of statements*
//! (`desugar::seals_schematic_scope`, `link_scope::statement_owner`),
//! because that engine already owns its body's links: a leader stays a leader,
//! and a pinless landing there is not the sheet's to resolve. **Existence** is
//! a different question: a part is drawn by the family wherever it sits, and
//! what SPEC 21 forbids is a family type *outside the scope* — a `|R|`
//! participating in a sequence drawn on a sheet is still on the sheet. Sealing
//! this walk too would make it an error, which is a language change no law
//! asks for.
//!
//! A part inside a sealed engine is still a **landing**, for the same reason
//! the sheet's own laws are endpoint-decided: being an addressed part is the
//! proof of scope, so a wire written outside the sheet lands on its pins like
//! any wire (`a_wire_from_outside_lands_on_a_sealed_engines_pin`). What the
//! seal stops is the *reading* of the statement, never the address.
//!
//! Everything downstream trusts this gate: past it a schematic part exists
//! only inside a schematic scope, so the router's fixed ports and `:side` ban
//! key on the **part** and never re-ask the scope
//! ([`crate::routing::ortho::request`]); and a chart type exists only where its
//! layout reads it, so each engine's child reader judges *composition* alone.

use super::{chart, schematic};
use crate::error::{Code, Error};
use crate::resolve::{AttrMap, Program, ResolvedInst};

/// The layout scopes a type can belong to, carried down the walk.
#[derive(Clone, Copy)]
struct Scope {
    schematic: bool,
    chart: bool,
    pie: bool,
}

impl Scope {
    /// The scope a container opens for its children — its own `layout:` added
    /// to whatever already encloses it.
    fn enter(self, attrs: &AttrMap) -> Self {
        Self {
            schematic: self.schematic || schematic::is_schematic(attrs),
            chart: self.chart || chart::is_chart(attrs),
            pie: self.pie || chart::is_pie(attrs),
        }
    }
}

pub(super) fn check_types(program: &Program) -> Result<(), Error> {
    let root = Scope {
        schematic: false,
        chart: false,
        pie: false,
    }
    .enter(&program.scene.attrs);
    walk(&program.scene.nodes, root)
}

fn walk(nodes: &[ResolvedInst], scope: Scope) -> Result<(), Error> {
    for n in nodes {
        if !scope.schematic
            && let Some(ty) = crate::desugar::schematic::schematic_type(&n.type_chain)
        {
            return Err(
                Error::at(n.span, format!("'|{ty}|' belongs in a 'layout: schematic'"))
                    .code(Code::SCHEMATIC_TYPE),
            );
        }
        if let Some(e) = chart::out_of_scope(n, scope.chart, scope.pie) {
            return Err(e);
        }
        walk(&n.children, scope.enter(&n.attrs))?;
    }
    Ok(())
}
