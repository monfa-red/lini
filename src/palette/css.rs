//! The **CSS named colours** [SPEC 2] — the one table that says whether a bare
//! word in a colour slot names a colour. A Lini colour is a hex literal, a
//! `--var` off the built-in palette ([`super`]), a builder call, or one of
//! these names; anything else is `invalid color 'X'`
//! ([`crate::validate`]).
//!
//! The set is CSS Color 4's named-colour list — the compiler emits the name
//! verbatim into the SVG, so the authority is what a renderer understands, not
//! what this palette curates: `--rose` is a Lini hue, `rosybrown` a CSS colour,
//! and only the second is a bare word.

/// The 148 CSS colour names, plus the two keywords a paint slot takes
/// (`transparent`, `currentcolor`). Compared ASCII-lowercased — CSS names are
/// case-insensitive, which is what lets `currentColor` read.
const NAMES: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "currentcolor",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "transparent",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

/// The non-colour idents a paint slot takes [SPEC 6/14.6]: `none` paints
/// nothing, and `auto` is the series' derive-my-edge sentinel.
const PAINT_KEYWORDS: &[&str] = &["none", "auto"];

/// Whether a bare word names a colour in a colour slot.
pub(crate) fn is_color_name(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    NAMES.contains(&lower.as_str()) || PAINT_KEYWORDS.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_unique() {
        let mut sorted = NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, NAMES, "keep the table sorted and duplicate-free");
    }

    #[test]
    fn names_read_case_insensitively() {
        assert!(is_color_name("cornflowerblue"));
        assert!(is_color_name("currentColor"));
        assert!(is_color_name("none"));
        // A Lini palette hue is a `--var`, never a bare word.
        assert!(!is_color_name("rose"));
        assert!(!is_color_name("xyzzy"));
    }
}
