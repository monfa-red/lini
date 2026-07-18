//! The luminance knockout mask [SPEC 17] — **the** mechanism for breaking a
//! stroked path under something else: a link label's cut box, a drawing
//! halo's crossing break [SPEC 15.7]. White (`.lini-cut-bg`) shows the path
//! over its padded region; black cut shapes punch the holes. Mask-based, not
//! painted, so the break holds on any background — over hatching, in dark
//! mode. An explicit `userSpaceOnUse` region is required, else a straight
//! path's near-flat bbox would shrink the default region to nothing and hide
//! the whole path.

use super::values::num;
use std::fmt::Write;

/// Open a knockout mask over `region = (x, y, w, h)`: the `<mask>` element
/// and its white background rect. The caller appends its black cut shapes
/// and closes with [`close`].
pub(super) fn open(id: &str, region: (f64, f64, f64, f64)) -> String {
    let (x, y, w, h) = region;
    format!(
        r#"<mask id="{id}" maskUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}"><rect class="lini-cut-bg" x="{}" y="{}" width="{}" height="{}"/>"#,
        num(x),
        num(y),
        num(w),
        num(h),
        num(x),
        num(y),
        num(w),
        num(h),
    )
}

/// A label's cut box — a black rect [SPEC 13].
pub(super) fn cut_rect(m: &mut String, rect: (f64, f64, f64, f64)) {
    write!(
        m,
        r#"<rect class="lini-cut" x="{}" y="{}" width="{}" height="{}"/>"#,
        num(rect.0),
        num(rect.1),
        num(rect.2),
        num(rect.3),
    )
    .unwrap();
}

/// A crossing halo's cut — the sub-polyline over the crossing, stroked wide
/// enough to sever the line it masks; `.lini-halo` carries the black (the
/// `|halo|` cascade hook restyles or removes it) [SPEC 15.7].
pub(super) fn cut_polyline(m: &mut String, points: &[(f64, f64)], width: f64) {
    let pts: Vec<String> = points
        .iter()
        .map(|(x, y)| format!("{},{}", num(*x), num(*y)))
        .collect();
    write!(
        m,
        r#"<polyline class="lini-halo" fill="none" stroke-width="{}" points="{}"/>"#,
        num(width),
        pts.join(" "),
    )
    .unwrap();
}

pub(super) fn close(m: &mut String) {
    m.push_str("</mask>");
}
