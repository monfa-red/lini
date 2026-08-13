//! Content-addressed names for everything Lini writes into a namespace the
//! **page** owns, not the figure: `<defs>` ids, an embedded asset's id prefix,
//! and the stylesheet's scope class [SPEC 18].
//!
//! A compiled SVG is self-contained, but inlining two of them into one HTML
//! document merges their id space and their CSS selector space. A per-document
//! counter (`lini-gradient-1`) or a shared selector head (`.lini .lini-block`)
//! then names *different* things identically, and the later figure wins: its
//! `url(#…)` resolves to the earlier figure's def, its structural defaults
//! repaint the earlier figure's template-styled nodes.
//!
//! No self-contained figure can guarantee a name nothing else on the page
//! holds. What it can guarantee is that **a shared name names the same thing**
//! — so derive every such name from the thing it names. Two figures then
//! collide only where their definitions are identical, and there sharing is
//! correct: `url(#…)` resolves to an equal def, a duplicated CSS rule is a
//! no-op. The outlined glyph ids (`lg1500-259`) were already built this way;
//! this is that rule stated once, for all of them.

/// FNV-1a over `bytes`, folded to 32 bits and hex-formatted — the tag every
/// name below carries. Hand-rolled and integer-only so a name is byte-stable
/// across runs, platforms, and toolchains, exactly as the rest of the output
/// is ([`crate::math`] keeps the float side of that promise).
fn tag(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (h ^ (h >> 32)) as u32)
}

/// The id a `<defs>` entry is published under: `lini-{kind}-{tag}`, where
/// `body` canonically identifies the definition. Both the mint site and the
/// emit site call this on the **interned** definition, so the reference and
/// the def it points at can never drift apart.
pub(crate) fn def_id(kind: &str, body: &str) -> String {
    format!("lini-{kind}-{}", tag(body.as_bytes()))
}

/// The prefix every id inside an embedded SVG asset is rewritten to wear
/// [SPEC 18] — from the asset's own bytes, which is already what the embedding
/// is deterministic in.
pub(crate) fn asset_prefix(bytes: &[u8]) -> String {
    format!("lini-a{}-", tag(bytes))
}

/// The class the root `<svg>` wears and every selector in its `<style>` is
/// headed by — from that stylesheet's own text, so two figures share a scope
/// only when their CSS is identical.
pub(crate) fn scope_class(stylesheet: &str) -> String {
    format!("lini-scope-{}", tag(stylesheet.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_differ_by_asset() {
        assert_ne!(
            asset_prefix(b"<svg><g id=\"a\"/></svg>"),
            asset_prefix(b"<svg><g id=\"b\"/></svg>")
        );
        assert_eq!(asset_prefix(b"same bytes"), asset_prefix(b"same bytes"));
    }

    #[test]
    fn a_name_is_stable_and_distinguishes_its_body() {
        assert_eq!(
            def_id("gradient", "linear 135"),
            def_id("gradient", "linear 135")
        );
        assert_ne!(
            def_id("gradient", "linear 135"),
            def_id("gradient", "linear 90")
        );
        assert_ne!(def_id("hatch", "45"), def_id("clip", "45"));
    }
}
