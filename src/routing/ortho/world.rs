//! World assembly (ROUTING.md Model steps 1–2): the ladder of containers a
//! link may route in, and the per-world channel graphs the whole diagram
//! searches over — built once, from node placement alone.
use super::*;

/// The worlds an edge may route in, innermost first: the endpoints' common
/// container, then every transparent ancestor up to the scene root — a tight
/// interior never walls in a link its ancestors would let out. Containment
/// links stay inside their container.
pub(super) fn world_ladder(a: &str, b: &str) -> Vec<String> {
    if SceneIndex::contains(a, b) || SceneIndex::contains(b, a) {
        return vec![SceneIndex::world_of(a, b)];
    }
    let mut w = SceneIndex::world_of(a, b);
    let mut out = vec![w.clone()];
    while !w.is_empty() {
        w = parent_path(&w);
        out.push(w.clone());
    }
    out
}

/// Build every routing world once: the distinct containers the orthogonal
/// requests reach (each request's `world_ladder`, sorted and deduped), each
/// decomposed into its channel graph. The root world (`""`) spans `bounds` —
/// the scene plus its canvas margin; an interior world is its own placed body.
/// Keep-outs are the world's direct children inflated by clearance `c`.
pub(super) fn build_worlds(index: &SceneIndex, reqs: &[EdgeReq], c: f64) -> Vec<World> {
    let mine = |r: &&EdgeReq| match r.routing {
        Strategy::Orthogonal => true,
        Strategy::Straight => false,
    };
    let bounds = index.bounds().inflate(2.0 * c + 20.0);

    let mut world_paths: Vec<String> = reqs
        .iter()
        .filter(mine)
        .flat_map(|r| world_ladder(&r.a_path, &r.b_path))
        .collect();
    world_paths.sort();
    world_paths.dedup();
    world_paths
        .into_iter()
        .map(|path| {
            let wb = if path.is_empty() {
                bounds
            } else {
                index.rect(&path).expect("world body placed")
            };
            let keepouts: Vec<Rect> = index
                .child_rects(&path)
                .iter()
                .map(|r| r.inflate(c))
                .collect();
            let graph = ChannelGraph::build(wb, &keepouts, path.is_empty());
            World { path, graph }
        })
        .collect()
}
