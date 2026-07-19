//! Resolve-time image assets [SPEC 7]: a local `src:` path resolves against
//! the source file's directory and its bytes are read **once, here** — so
//! layout and render stay pure and the output is deterministic from the bytes.
//! An SVG asset is rewritten for embedding (every id prefixed `lini-aN-`, every
//! internal reference following — [SPEC 17]); a raster folds to a base64 data
//! URI. HTTP(S) URLs and authored `data:` URIs pass through untouched — the
//! compiler never touches the network. Under `lini serve` reads are confined
//! to the served root [SPEC 19].

use super::ir::{AttrMap, ResolvedValue};
use crate::error::{Code, Error};
use crate::span::Span;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::ops::Range;
use std::path::PathBuf;

/// Where a compile's local image paths resolve [SPEC 7/19]: `base_dir` is the
/// source file's directory (`None` ⇒ paths resolve as written, the stdin
/// case), `root` the serve traversal boundary (`None` ⇒ unbounded — a plain
/// CLI compile of your own file).
#[derive(Clone, Debug, Default)]
pub struct AssetEnv {
    pub base_dir: Option<PathBuf>,
    pub root: Option<PathBuf>,
}

/// Per-compile asset state: the environment plus the 1-based document-order
/// counter behind the `lini-aN-` id prefix [SPEC 17].
pub struct AssetState {
    env: AssetEnv,
    counter: Cell<usize>,
}

impl AssetState {
    pub fn new(env: AssetEnv) -> Self {
        Self {
            env,
            counter: Cell::new(0),
        }
    }
}

/// Resolve an `|image|`'s `src:` in place [SPEC 7]. URLs and authored data
/// URIs stay untouched; a local path is read now (the one read), classified by
/// content, and folded into the attrs: an SVG asset lands as `embed-svg` (the
/// rewritten inner markup) + `embed-attrs` (its kept root attributes), a
/// raster replaces `src` with a base64 data URI. Missing, escaping, or
/// unrecognisable assets error at the `src:` span.
pub fn embed_image(attrs: &mut AttrMap, state: &AssetState, span: Span) -> Result<(), Error> {
    let Some(ResolvedValue::String(src)) = attrs.get("src") else {
        return Ok(());
    };
    if is_pass_through(src) {
        return Ok(());
    }
    let src = src.clone();

    let full = match &state.env.base_dir {
        Some(base) => base.join(&src),
        None => PathBuf::from(&src),
    };
    // The serve boundary [SPEC 19]: canonicalize (resolving symlinks) and
    // require the asset inside the served root. Checked before the read — a
    // file outside the boundary is never opened. A path that cannot
    // canonicalize does not exist, which is the read error's domain.
    if let Some(root) = &state.env.root
        && let Ok(canon) = full.canonicalize()
        && !root
            .canonicalize()
            .is_ok_and(|canon_root| canon.starts_with(canon_root))
    {
        return Err(
            Error::at(span, format!("'{src}' resolves outside the served root"))
                .code(Code::ASSET_ESCAPES_ROOT),
        );
    }
    let bytes = std::fs::read(&full).map_err(|e| {
        let why = match e.kind() {
            std::io::ErrorKind::NotFound => "no such file".to_string(),
            std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
            _ => e.to_string(),
        };
        Error::at(span, format!("cannot read image '{src}' — {why}")).code(Code::ASSET_NOT_FOUND)
    })?;

    let n = state.counter.get() + 1;
    state.counter.set(n);

    if let Some(text) = sniff_svg(&bytes) {
        let prefix = format!("lini-a{n}-");
        let (root_attrs, inner) = rewrite_svg(text, &prefix).map_err(|why| {
            Error::at(span, format!("cannot read image '{src}' — {why}"))
                .code(Code::ASSET_NOT_FOUND)
        })?;
        attrs.remove("src");
        attrs.insert("embed-attrs", ResolvedValue::String(root_attrs));
        attrs.insert("embed-svg", ResolvedValue::String(inner));
        return Ok(());
    }
    if let Some(mime) = raster_mime(&bytes) {
        let uri = format!("data:{mime};base64,{}", base64(&bytes));
        attrs.insert("src", ResolvedValue::String(uri));
        return Ok(());
    }
    Err(Error::at(
        span,
        format!("cannot read image '{src}' — not an SVG or raster (PNG/JPEG/GIF/WebP)"),
    )
    .code(Code::ASSET_NOT_FOUND))
}

/// The authored non-embedded forms [SPEC 7]: HTTP(S) URLs and `data:` URIs
/// emit unchanged.
fn is_pass_through(src: &str) -> bool {
    let lower = src.get(..8).unwrap_or(src).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("data:")
}

/// The raster MIME by magic number — PNG / JPEG / GIF / WebP [SPEC 7].
fn raster_mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Some("image/gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

/// Whether the bytes are an SVG document: valid UTF-8 whose first element
/// (past BOM, prolog, comments) is `<svg>`. Returns the text on a hit.
fn sniff_svg(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes)
        .ok()?
        .trim_start_matches('\u{feff}');
    root_svg(text)?;
    Some(text)
}

// ───────────────────────── The id rewrite [SPEC 17] ─────────────────────────

/// One scanned piece the rewriter cares about: an attribute value (quote-free
/// range) or a character-data run between tags (`<style>` text and CDATA
/// included — `url(#…)` references live there too).
enum Seg {
    Attr {
        name: Range<usize>,
        value: Range<usize>,
    },
    Text(Range<usize>),
}

/// A minimal, quote-aware scan of XML text into [`Seg`]s. Comments, prologs,
/// and DOCTYPEs are opaque (copied verbatim by the rewriter); everything else
/// is an element tag's attributes or character data.
fn scan(text: &str) -> Vec<Seg> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            let rest = &text[i..];
            if let Some(skip) = opaque_len(rest) {
                i += skip;
            } else if let Some(inner) = rest.strip_prefix("<![CDATA[") {
                let len = inner.find("]]>").unwrap_or(inner.len());
                out.push(Seg::Text(i + 9..i + 9 + len));
                i += 9 + len + 3.min(inner.len() - len);
            } else {
                i = scan_tag(text, i, &mut out);
            }
        } else {
            let start = i;
            while i < b.len() && b[i] != b'<' {
                i += 1;
            }
            out.push(Seg::Text(start..i));
        }
    }
    out
}

/// The length of an opaque piece (`<!-- -->`, `<? ?>`, `<!DOCTYPE >`) at the
/// start of `rest`, or `None` when an element tag (or CDATA) begins here.
fn opaque_len(rest: &str) -> Option<usize> {
    if rest.starts_with("<!--") {
        Some(rest.find("-->").map(|j| j + 3).unwrap_or(rest.len()))
    } else if rest.starts_with("<?") {
        Some(rest.find("?>").map(|j| j + 2).unwrap_or(rest.len()))
    } else if rest.starts_with("<![CDATA[") || !rest.starts_with("<!") {
        None
    } else {
        Some(rest.find('>').map(|j| j + 1).unwrap_or(rest.len()))
    }
}

/// Scan one element tag from `open` (its `<`), pushing attribute segments;
/// returns the index just past the closing `>`.
fn scan_tag(text: &str, open: usize, out: &mut Vec<Seg>) -> usize {
    let b = text.as_bytes();
    let mut i = open + 1;
    if i < b.len() && b[i] == b'/' {
        i += 1;
    }
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return i;
        }
        if b[i] == b'>' {
            return i + 1;
        }
        if b[i] == b'/' {
            i += 1;
            continue;
        }
        let name_start = i;
        while i < b.len()
            && b[i] != b'='
            && !b[i].is_ascii_whitespace()
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        let name = name_start..i;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            continue; // valueless attribute
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
            let q = b[i];
            let vstart = i + 1;
            i = vstart;
            while i < b.len() && b[i] != q {
                i += 1;
            }
            out.push(Seg::Attr {
                name,
                value: vstart..i,
            });
            i = (i + 1).min(b.len());
        } else {
            let vstart = i;
            while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                i += 1;
            }
            out.push(Seg::Attr {
                name,
                value: vstart..i,
            });
        }
    }
}

/// The root `<svg …>` open tag's range and whether it self-closes, or `None`
/// when the first element is not `<svg>`.
fn root_svg(text: &str) -> Option<(Range<usize>, bool)> {
    let mut i = 0;
    let b = text.as_bytes();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
        } else if b[i] == b'<' {
            match opaque_len(&text[i..]) {
                Some(skip) => i += skip,
                None => break,
            }
        } else {
            return None;
        }
    }
    let rest = &text[i..];
    let named = rest.strip_prefix("<svg")?;
    if !named.starts_with(['>', '/']) && !named.starts_with(|c: char| c.is_ascii_whitespace()) {
        return None;
    }
    // Find the tag's closing `>`, quote-aware.
    let rb = rest.as_bytes();
    let mut j = 1;
    while j < rb.len() && rb[j] != b'>' {
        if rb[j] == b'"' || rb[j] == b'\'' {
            let q = rb[j];
            j += 1;
            while j < rb.len() && rb[j] != q {
                j += 1;
            }
        }
        j += 1;
    }
    if j >= rb.len() {
        return None;
    }
    let self_closing = rb[j - 1] == b'/';
    Some((i..i + j + 1, self_closing))
}

/// Rewrite an SVG asset for embedding [SPEC 17]: every declared `id` gains
/// `prefix`, and every internal reference follows — `url(#…)` in attributes,
/// inline `style`, and `<style>` text; fragment `href` / `xlink:href`.
/// Returns `(root attributes to keep, rewritten inner markup)`: the asset's
/// placement attributes (`width`/`height`/`x`/`y`/`preserveAspectRatio`) are
/// dropped — render owns the mapping into the node box — with `viewBox`
/// synthesized from the dropped width × height when the asset states none.
fn rewrite_svg(text: &str, prefix: &str) -> Result<(String, String), String> {
    let (tag, self_closing) = root_svg(text).ok_or("not an SVG")?;
    let inner_range = if self_closing {
        tag.end..tag.end
    } else {
        let close = text.rfind("</svg").ok_or("unclosed <svg>")?;
        if close < tag.end {
            return Err("unclosed <svg>".to_string());
        }
        tag.end..close
    };
    // Every id the asset declares, root tag included [SPEC 17]. Only
    // references to these rewrite — an external fragment stays as authored.
    let ids: BTreeSet<&str> = scan(text)
        .iter()
        .filter_map(|s| match s {
            Seg::Attr { name, value } if &text[name.clone()] == "id" => Some(&text[value.clone()]),
            _ => None,
        })
        .collect();
    let inner = rewrite_fragment(&text[inner_range], prefix, &ids);
    let root_attrs = rebuild_root_attrs(&text[tag], prefix, &ids);
    Ok((root_attrs, inner))
}

/// Rewrite one markup fragment: attribute values and character-data runs pass
/// through [`rewrite_attr_value`] / [`rewrite_url_refs`]; everything between
/// stays byte-identical.
fn rewrite_fragment(text: &str, prefix: &str, ids: &BTreeSet<&str>) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut last = 0;
    for seg in scan(text) {
        let (range, new) = match seg {
            Seg::Attr { name, value } => {
                let rewritten = rewrite_attr_value(&text[name], &text[value.clone()], prefix, ids);
                (value, rewritten)
            }
            Seg::Text(range) => {
                let rewritten = rewrite_url_refs(&text[range.clone()], prefix, ids);
                (range, rewritten)
            }
        };
        if let Some(new) = new {
            out.push_str(&text[last..range.start]);
            out.push_str(&new);
            last = range.end;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// One attribute's rewritten value, or `None` when it is untouched: a declared
/// `id` gains the prefix, a fragment `href`/`xlink:href` to a declared id
/// follows, and any other value rewrites its `url(#…)` references (the inline
/// `style` case rides this).
fn rewrite_attr_value(
    name: &str,
    value: &str,
    prefix: &str,
    ids: &BTreeSet<&str>,
) -> Option<String> {
    if name == "id" && ids.contains(value) {
        return Some(format!("{prefix}{value}"));
    }
    if (name == "href" || name == "xlink:href")
        && let Some(frag) = value.strip_prefix('#')
        && ids.contains(frag)
    {
        return Some(format!("#{prefix}{frag}"));
    }
    rewrite_url_refs(value, prefix, ids)
}

/// Rewrite every `url(#id)` (quoted forms included) whose fragment is a
/// declared id; `None` when nothing matched.
fn rewrite_url_refs(text: &str, prefix: &str, ids: &BTreeSet<&str>) -> Option<String> {
    let mut out = String::new();
    let mut last = 0;
    let mut i = 0;
    while let Some(found) = text[i..].find("url(") {
        let start = i + found + 4;
        let rest = &text[start..];
        let end = rest.find(')').unwrap_or(rest.len());
        let body = &rest[..end];
        let frag = body.trim().trim_matches(['"', '\'']);
        if let Some(id) = frag.strip_prefix('#')
            && ids.contains(id)
        {
            // Replace the fragment only, keeping the body's quoting.
            let frag_at = start + body.find('#').unwrap_or(0);
            out.push_str(&text[last..frag_at]);
            out.push('#');
            out.push_str(prefix);
            out.push_str(id);
            last = frag_at + 1 + id.len();
        }
        i = start + end;
    }
    if last == 0 {
        return None;
    }
    out.push_str(&text[last..]);
    Some(out)
}

/// The root attributes an embedded asset keeps, rebuilt as one string:
/// placement (`width`/`height`/`x`/`y`/`preserveAspectRatio`) and the
/// redundant plain `xmlns` drop; everything else — `viewBox`, paints, `class`,
/// `xmlns:*` — stays, its value rewritten like any attribute. Without a
/// `viewBox` the dropped width × height synthesize one, so `fit:` can map the
/// content into the node box.
fn rebuild_root_attrs(tag: &str, prefix: &str, ids: &BTreeSet<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let (mut width, mut height, mut has_viewbox) = (None, None, false);
    for seg in scan(tag) {
        if let Seg::Attr { name, value } = seg {
            let n = &tag[name];
            let v = &tag[value];
            match n {
                "width" => width = css_length(v),
                "height" => height = css_length(v),
                "x" | "y" | "preserveAspectRatio" | "xmlns" => {}
                _ => {
                    if n == "viewBox" {
                        has_viewbox = true;
                    }
                    let v = rewrite_attr_value(n, v, prefix, ids).unwrap_or_else(|| v.to_string());
                    parts.push(format!("{n}=\"{}\"", v.replace('"', "&quot;")));
                }
            }
        }
    }
    if !has_viewbox && let (Some(w), Some(h)) = (width, height) {
        parts.insert(0, format!("viewBox=\"0 0 {w} {h}\""));
    }
    parts.join(" ")
}

/// A numeric CSS length (`"48"`, `"48px"`) as its bare number string —
/// percentages and units other than px carry no intrinsic size.
fn css_length(v: &str) -> Option<&str> {
    let bare = v.trim().trim_end_matches("px");
    bare.parse::<f64>().ok().map(|_| bare)
}

/// Plain base64 (RFC 4648, padded) — a data: URL needs nothing more, and a
/// dependency would be heavier than the 20 lines. Shared with the font
/// embedder [SPEC 17].
pub(crate) fn base64(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(TBL[(n >> 18) as usize & 63] as char);
        out.push(TBL[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TBL[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TBL[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite(text: &str) -> (String, String) {
        rewrite_svg(text, "lini-a1-").expect("rewrite")
    }

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn ids_and_url_refs_rewrite() {
        let (_, inner) = rewrite(
            r##"<svg viewBox="0 0 4 4"><defs><linearGradient id="g"/></defs><rect fill="url(#g)"/></svg>"##,
        );
        assert!(inner.contains(r#"id="lini-a1-g""#), "{inner}");
        assert!(inner.contains("url(#lini-a1-g)"), "{inner}");
    }

    #[test]
    fn fragment_hrefs_rewrite_in_both_spellings() {
        let (_, inner) =
            rewrite(r##"<svg><circle id="dot"/><use href="#dot"/><use xlink:href="#dot"/></svg>"##);
        assert!(inner.contains(r##"href="#lini-a1-dot""##), "{inner}");
        assert!(inner.contains(r##"xlink:href="#lini-a1-dot""##), "{inner}");
    }

    #[test]
    fn inline_style_and_style_element_urls_rewrite() {
        let (_, inner) = rewrite(
            r##"<svg><g id="p"/><rect style="fill: url(#p)"/><style>.a { fill: url(#p); }</style></svg>"##,
        );
        assert!(
            inner.contains(r##"style="fill: url(#lini-a1-p)""##),
            "{inner}"
        );
        assert!(inner.contains(".a { fill: url(#lini-a1-p); }"), "{inner}");
    }

    #[test]
    fn substring_ids_do_not_cross_rewrite() {
        // `g` must not rewrite inside `url(#gr)` — fragments match whole.
        let (_, inner) = rewrite(
            r##"<svg><g id="g"/><g id="gr"/><rect fill="url(#gr)" stroke="url(#g)"/></svg>"##,
        );
        assert!(inner.contains("url(#lini-a1-gr)"), "{inner}");
        assert!(inner.contains("url(#lini-a1-g)"), "{inner}");
        assert!(!inner.contains("url(#lini-a1-lini"), "{inner}");
    }

    #[test]
    fn external_fragments_stay_as_authored() {
        let (_, inner) = rewrite(r##"<svg><use href="#elsewhere"/></svg>"##);
        assert!(inner.contains(r##"href="#elsewhere""##), "{inner}");
    }

    #[test]
    fn root_attrs_drop_placement_and_keep_viewbox() {
        let (root, _) = rewrite(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48" fill="none"><rect/></svg>"#,
        );
        assert_eq!(root, r#"viewBox="0 0 48 48" fill="none""#);
    }

    #[test]
    fn missing_viewbox_synthesizes_from_width_height() {
        let (root, _) = rewrite(r#"<svg width="24px" height="16"><rect/></svg>"#);
        assert_eq!(root, r#"viewBox="0 0 24 16""#);
    }

    #[test]
    fn prolog_comment_and_doctype_are_skipped() {
        let (root, inner) = rewrite(
            "<?xml version=\"1.0\"?>\n<!-- logo -->\n<!DOCTYPE svg>\n<svg viewBox=\"0 0 1 1\"><g id=\"a\"/></svg>",
        );
        assert_eq!(root, r#"viewBox="0 0 1 1""#);
        assert!(inner.contains(r#"id="lini-a1-a""#), "{inner}");
    }

    #[test]
    fn two_assets_get_distinct_prefixes() {
        let doc = r##"<svg><g id="g"/><use href="#g"/></svg>"##;
        let (_, one) = rewrite_svg(doc, "lini-a1-").unwrap();
        let (_, two) = rewrite_svg(doc, "lini-a2-").unwrap();
        assert!(one.contains("lini-a1-g") && !one.contains("lini-a2-g"));
        assert!(two.contains("lini-a2-g") && !two.contains("lini-a1-g"));
    }

    #[test]
    fn raster_magic_classifies() {
        assert_eq!(
            raster_mime(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
            Some("image/png")
        );
        assert_eq!(raster_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(raster_mime(b"GIF89a...."), Some("image/gif"));
        assert_eq!(
            raster_mime(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some("image/webp")
        );
        assert_eq!(raster_mime(b"plain text"), None);
    }

    #[test]
    fn non_svg_first_element_is_not_svg() {
        assert!(sniff_svg(b"<html><svg/></html>").is_none());
        assert!(sniff_svg(b"\xff\xfe binary").is_none());
        assert!(sniff_svg(b"<!-- c --><svg/>").is_some());
    }
}
