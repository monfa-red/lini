//! The dialect's four quarters [SPEC 15.11]: the **vocabulary gate** (a
//! floorplan scope is a drawing scope, and its own types are legal only here),
//! the **wall** (a centreline grown into its mitred poché outline), the
//! **openings** stationed on it, and the **fixtures** that furnish it. One
//! helper set, one law per module.

mod face;
mod fixture;
mod gate;
mod opening;
mod wall;

use crate::layout::LaidOut;
use crate::layout::drawing::testutil::by_id;

/// A wall long enough to be the geometry child every drawing scope needs.
const WALL: &str = "|wall#w| { draw: move(0, 0) right(4000):north down(3000):east; }\n";

fn scope(body: &str) -> String {
    format!("|floorplan#f| [\n{body}]\n")
}

/// The placed wall's drawn path — the offset outline, post-fold.
fn wall_path(l: &LaidOut, id: &str) -> String {
    match by_id(&l.nodes, id).attrs.get("path") {
        Some(crate::resolve::ResolvedValue::String(d)) => d.clone(),
        other => panic!("no folded path on the wall: {other:?}"),
    }
}
