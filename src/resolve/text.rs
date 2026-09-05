//! The text bake [SPEC 6]: `text-transform` rewrites a run's content the
//! moment it resolves, so the box is measured from the glyphs it will draw
//! — one rewrite, before layout, for a node's text and a link's label alike.

use super::{AttrMap, ResolvedValue};

/// `text` as its resolved `text-transform` draws it.
pub(crate) fn transformed(text: &str, attrs: &AttrMap) -> String {
    match attrs.get("text-transform") {
        Some(ResolvedValue::Ident(t)) if t == "uppercase" => text.to_uppercase(),
        Some(ResolvedValue::Ident(t)) if t == "lowercase" => text.to_lowercase(),
        Some(ResolvedValue::Ident(t)) if t == "capitalize" => text
            .split_inclusive(char::is_whitespace)
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                }
            })
            .collect(),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(transform: &str) -> AttrMap {
        let mut attrs = AttrMap::new();
        attrs.insert("text-transform", ResolvedValue::Ident(transform.into()));
        attrs
    }

    #[test]
    fn each_transform_rewrites_the_run() {
        assert_eq!(transformed("ab cd", &with("uppercase")), "AB CD");
        assert_eq!(transformed("AB CD", &with("lowercase")), "ab cd");
        assert_eq!(transformed("ab cd\nef", &with("capitalize")), "Ab Cd\nEf");
        assert_eq!(transformed("ab cd", &with("none")), "ab cd");
        assert_eq!(transformed("ab cd", &AttrMap::new()), "ab cd");
    }
}
