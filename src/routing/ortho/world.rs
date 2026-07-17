//! World assembly (ROUTING.md Model steps 1–2): the ladder of containers a
//! link may route in, and the per-world channel graphs the whole diagram
//! searches over — built once, from node placement alone. Worlds are keyed
//! by **scene-node identity** ([`WorldKey`]), never by path — an anonymous
//! container's interior is a routing world exactly like a named one's.
use super::*;
use scene::WorldKey;

/// The worlds an edge may route in, innermost first: the endpoints' common
/// container, then every transparent ancestor up to the scene root — a tight
/// interior never walls in a link its ancestors would let out. A **geometric**
/// containment link (the inner endpoint actually inside the outer) stays inside
/// its container; a mere descendant placed beside its ancestor (a tree's
/// branch) climbs the ancestor ladder like an ordinary sibling wire.
pub(super) fn world_ladder(index: &SceneIndex, a: &str, b: &str) -> Vec<WorldKey> {
    if index.geo_contains(a, b) || index.geo_contains(b, a) {
        return vec![index.world_of(a, b)];
    }
    let mut out = vec![index.common_world(a, b)];
    while let Some(up) = index.parent_world(*out.last().expect("non-empty")) {
        out.push(up);
    }
    out
}

/// Build every routing world once: the distinct containers the orthogonal
/// requests reach (each request's `world_ladder`, sorted and deduped), each
/// decomposed into its channel graph. The root world (`None`) spans `bounds` —
/// the scene plus its canvas margin; an interior world is its own placed body.
/// Keep-outs are the world's direct children inflated by clearance `c`.
pub(super) fn build_worlds(index: &SceneIndex, reqs: &[EdgeReq], c: f64) -> Vec<World> {
    let bounds = index.bounds().inflate(2.0 * c + 20.0);

    let mut keys: Vec<WorldKey> = reqs
        .iter()
        .filter(|r| r.routing == crate::resolve::Strategy::Orthogonal)
        .flat_map(|r| world_ladder(index, &r.a_path, &r.b_path))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter()
        .map(|key| {
            let wb = index.world_rect(key).unwrap_or(bounds);
            let keepouts: Vec<Rect> = index
                .child_rects(key)
                .iter()
                .map(|r| r.inflate(c))
                .collect();
            let graph = ChannelGraph::build(wb, &keepouts, key.is_none());
            World { key, graph }
        })
        .collect()
}
