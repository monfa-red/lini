//! The built-in named-hue palette (SPEC §11.2): 11 hues × 4 job-named tiers,
//! generated from OKLCH seeds so the ramp is perceptually even and the hues read as
//! a family. Each tier is a `light-dark(#light, #dark)` literal — themeable like any
//! `--lini-*` var, flipping with the mode, baking for resvg. Aliases catch the
//! conventional names this curated set renames or drops.
//!
//! Re-tuning the whole palette is editing two tables: [`HUES`] (one seed per hue) and
//! [`TIERS`] (the lightness/chroma targets per tier per mode). The data is the design.

pub(crate) mod oklch;

use crate::resolve::{ResolvedCall, ResolvedValue};

/// A hue seed: its name, OKLCH hue angle (degrees), and a chroma multiplier
/// (grey = 0). The multiplier trims hues whose gamut is narrow (yellows) and gives
/// the roomy ones (blues) a touch more room — the tier chroma is otherwise shared.
struct Hue {
    name: &'static str,
    hue: f64,
    chroma: f64,
}

/// The eleven hues, warm → cool, grey last. `red` stays clear for danger; `rose` is
/// the pretty warm-pink (its light tiers are the pinks); `green` is an emerald off
/// the muddy middle, `lime` the lemony one; `purple` leans blue (indigo + violet).
const HUES: &[Hue] = &[
    Hue {
        name: "red",
        hue: 28.0,
        chroma: 1.00,
    },
    Hue {
        name: "rose",
        hue: 6.0,
        chroma: 1.00,
    },
    Hue {
        name: "orange",
        hue: 55.0,
        chroma: 1.00,
    },
    Hue {
        name: "amber",
        hue: 80.0,
        chroma: 1.08,
    },
    Hue {
        name: "lime",
        hue: 120.0,
        chroma: 1.02,
    },
    Hue {
        name: "green",
        hue: 158.0,
        chroma: 1.00,
    },
    Hue {
        name: "teal",
        hue: 188.0,
        chroma: 1.00,
    },
    Hue {
        name: "sky",
        hue: 222.0,
        chroma: 1.00,
    },
    Hue {
        name: "blue",
        hue: 255.0,
        chroma: 1.05,
    },
    Hue {
        name: "purple",
        hue: 285.0,
        chroma: 1.05,
    },
    Hue {
        name: "gray",
        hue: 250.0,
        chroma: 0.00,
    },
];

/// One tier: its name suffix (empty = the bare hero) and OKLCH `(L, C)` targets for
/// each mode. Light mode descends in lightness wash → ink; dark mode keeps each
/// tier's *job* — `ink` bright (text/lines), `wash` a deep, muted surface.
struct Tier {
    suffix: &'static str,
    light: (f64, f64),
    dark: (f64, f64),
}

const TIERS: &[Tier] = &[
    Tier {
        suffix: "-wash",
        light: (0.95, 0.022),
        dark: (0.290, 0.040),
    },
    Tier {
        suffix: "-soft",
        light: (0.86, 0.072),
        dark: (0.360, 0.058),
    },
    Tier {
        suffix: "",
        light: (0.72, 0.127),
        dark: (0.620, 0.115),
    },
    Tier {
        suffix: "-ink",
        light: (0.520, 0.150),
        dark: (0.800, 0.120),
    },
];

/// Conventional names this palette renames or drops → the hue they resolve to, so
/// muscle memory still lands (`--yellow` → `--amber`, …). Tree-shaking follows the
/// pointer, so an unused alias costs nothing.
const ALIASES: &[(&str, &str)] = &[
    ("yellow", "amber"),
    ("pink", "rose"),
    ("indigo", "purple"),
    ("cyan", "teal"),
];

/// Every palette variable as `(name_without_lini_prefix, value)`: each hue's four
/// tiers as `light-dark()` literals, then the aliases. Appended to the built-in
/// defaults (SPEC §11.1) by [`crate::resolve::built_in_defaults`].
pub fn palette_vars() -> Vec<(String, ResolvedValue)> {
    let mut out = Vec::with_capacity(HUES.len() * TIERS.len() + ALIASES.len());
    for hue in HUES {
        for tier in TIERS {
            let arm = |(l, c): (f64, f64)| {
                ResolvedValue::Hex(oklch::oklch_to_hex(l, c * hue.chroma, hue.hue))
            };
            out.push((
                format!("{}{}", hue.name, tier.suffix),
                light_dark(arm(tier.light), arm(tier.dark)),
            ));
        }
    }
    for (alias, target) in ALIASES {
        out.push((
            alias.to_string(),
            ResolvedValue::LiveVar {
                name: (*target).into(),
                raw: false,
            },
        ));
    }
    out
}

fn light_dark(light: ResolvedValue, dark: ResolvedValue) -> ResolvedValue {
    ResolvedValue::Call(ResolvedCall {
        name: "light-dark".into(),
        args: vec![light, dark],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vec<(String, ResolvedValue)> {
        palette_vars()
    }

    #[test]
    fn every_hue_has_four_tiers_plus_aliases() {
        let v = vars();
        assert_eq!(v.len(), HUES.len() * TIERS.len() + ALIASES.len());
        for suffix in ["-wash", "-soft", "", "-ink"] {
            assert!(
                v.iter().any(|(n, _)| *n == format!("teal{suffix}")),
                "missing teal{suffix}"
            );
        }
    }

    #[test]
    fn tiers_are_light_dark_literals() {
        let v = vars();
        let (_, teal) = v.iter().find(|(n, _)| n == "teal").unwrap();
        match teal {
            ResolvedValue::Call(c) => {
                assert_eq!(c.name, "light-dark");
                assert!(matches!(c.args[0], ResolvedValue::Hex(_)));
                assert!(matches!(c.args[1], ResolvedValue::Hex(_)));
            }
            other => panic!("expected light-dark(), got {other:?}"),
        }
    }

    #[test]
    fn aliases_point_at_their_hue() {
        let v = vars();
        let (_, yellow) = v.iter().find(|(n, _)| n == "yellow").unwrap();
        assert!(matches!(yellow, ResolvedValue::LiveVar { name, .. } if name == "amber"));
    }

    #[test]
    fn grey_is_neutral() {
        // Zero chroma → equal channels in both arms.
        let v = vars();
        let (_, gray) = v.iter().find(|(n, _)| n == "gray").unwrap();
        let ResolvedValue::Call(c) = gray else {
            panic!()
        };
        for arm in &c.args {
            let ResolvedValue::Hex(h) = arm else { panic!() };
            assert_eq!(&h[0..2], &h[2..4], "grey not neutral: {h}");
            assert_eq!(&h[2..4], &h[4..6], "grey not neutral: {h}");
        }
    }
}
