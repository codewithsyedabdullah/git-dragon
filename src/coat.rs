//! Language-driven colouring.
//!
//! A [`DragonCoat`] says how the dragon's three colour zones are tinted:
//!
//! | Zone      | Language       | Parts                                   |
//! |-----------|----------------|-----------------------------------------|
//! | primary   | most used      | scales, belly, body highlights          |
//! | secondary | second most    | wing membranes                          |
//! | tertiary  | third most     | horns and crest highlights              |
//!
//! Each zone has a *strength*. It grows with the language's share of the account, so a
//! profile dominated by one language is tinted strongly while a mixed profile keeps more of
//! the original emerald/turquoise art. Without usable language data the coat is `None` and
//! the sprites are drawn untouched (the canonical emerald dragon).

use crate::history::{AccountSnapshot, LanguageShare};

/// Four brightness steps of one colour: `[shadow, mid_dark, mid_light, highlight]`.
pub type Ramp = [[f32; 3]; 4];

/// A tint applied to one colour zone of the dragon.
#[derive(Clone, Debug, PartialEq)]
pub struct Tint {
    /// Source colour (the language colour, brightness-clamped so the dragon stays readable).
    pub base: [u8; 3],
    /// Shading ramp built from `base`.
    pub ramp: Ramp,
    /// 0.0 keeps the original art, 1.0 fully replaces the hue.
    pub strength: f32,
    /// Name of the language this zone came from.
    pub language: String,
}

/// Complete colouring for one dragon.
#[derive(Clone, Debug, PartialEq)]
pub struct DragonCoat {
    pub primary: Tint,
    pub secondary: Tint,
    pub tertiary: Tint,
}

/// Parse `#rrggbb` (the `#` is optional).
pub fn parse_hex_color(hex: &str) -> Option<[u8; 3]> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 || !clean.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
    let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
    let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
    Some([r, g, b])
}

/// Build the four-step ramp for a base colour.
pub fn ramp_from_rgb(base: [u8; 3]) -> Ramp {
    let b = base.map(f32::from);
    let mul = |k: f32| [b[0] * k, b[1] * k, b[2] * k];
    let lift = |k: f32| {
        [
            b[0] + (255.0 - b[0]) * k,
            b[1] + (255.0 - b[1]) * k,
            b[2] + (255.0 - b[2]) * k,
        ]
    };
    [mul(0.22), mul(0.55), b, lift(0.5)]
}

/// Lighten colours that would swallow the dragon's dark outlines (e.g. Lua navy, C grey).
fn readable(base: [u8; 3]) -> [u8; 3] {
    let luma = 0.299 * f32::from(base[0]) + 0.587 * f32::from(base[1]) + 0.114 * f32::from(base[2]);
    const MIN_LUMA: f32 = 80.0;
    if luma >= MIN_LUMA {
        return base;
    }
    let k = ((MIN_LUMA - luma) / (255.0 - luma)).clamp(0.0, 1.0);
    base.map(|c| (f32::from(c) + (255.0 - f32::from(c)) * k).round() as u8)
}

impl Tint {
    fn new(language: &str, base: [u8; 3], strength: f32) -> Self {
        let base = readable(base);
        Self {
            base,
            ramp: ramp_from_rgb(base),
            strength: strength.clamp(0.0, 1.0),
            language: language.to_owned(),
        }
    }
}

impl DragonCoat {
    /// Build a coat from languages ordered most-used first.
    ///
    /// Entries with an unparsable colour are skipped. Returns `None` when nothing usable is
    /// left, which means "draw the canonical emerald dragon".
    pub fn from_languages(languages: &[LanguageShare]) -> Option<Self> {
        let usable: Vec<(&LanguageShare, [u8; 3])> = languages
            .iter()
            .filter_map(|l| parse_hex_color(&l.hex_color).map(|rgb| (l, rgb)))
            .take(3)
            .collect();
        let (last_lang, last_rgb) = *usable.last()?;

        // Normalised share of each of the (up to) three languages. A language with a zero
        // repo count still counts as one so a hand-written snapshot cannot divide by zero.
        let weights: Vec<f32> = usable
            .iter()
            .map(|(l, _)| l.repo_count.max(1) as f32)
            .collect();
        let total: f32 = weights.iter().sum();
        let share = |i: usize| weights.get(i).map_or(weights[weights.len() - 1], |w| *w) / total;

        let pick = |i: usize| usable.get(i).map_or((last_lang, last_rgb), |p| *p);
        let (l1, c1) = pick(0);
        let (l2, c2) = pick(1);
        let (l3, c3) = pick(2);

        Some(Self {
            primary: Tint::new(&l1.name, c1, 0.55 + 0.40 * share(0)),
            secondary: Tint::new(&l2.name, c2, 0.45 + 0.50 * share(1)),
            tertiary: Tint::new(&l3.name, c3, 0.50 + 0.45 * share(2)),
        })
    }

    /// Coat for a snapshot. `None` when `canon_only` is set or no language colour is usable.
    pub fn from_snapshot(snapshot: &AccountSnapshot, canon_only: bool) -> Option<Self> {
        if canon_only {
            return None;
        }
        Self::from_languages(&snapshot.languages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(name: &str, hex: &str, repos: u32) -> LanguageShare {
        LanguageShare {
            name: name.to_owned(),
            hex_color: hex.to_owned(),
            repo_count: repos,
        }
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex_color("#3178c6"), Some([0x31, 0x78, 0xc6]));
        assert_eq!(parse_hex_color("dea584"), Some([0xde, 0xa5, 0x84]));
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#zzzzzz"), None);
        assert_eq!(parse_hex_color("ééé"), None);
    }

    #[test]
    fn no_usable_language_means_canon() {
        assert!(DragonCoat::from_languages(&[]).is_none());
        assert!(DragonCoat::from_languages(&[lang("X", "nope", 3)]).is_none());
    }

    #[test]
    fn zones_follow_language_order() {
        let coat = DragonCoat::from_languages(&[
            lang("Python", "#3572A5", 6),
            lang("JavaScript", "#f1e05a", 3),
            lang("C++", "#f34b7d", 1),
        ])
        .expect("coat");
        assert_eq!(coat.primary.language, "Python");
        assert_eq!(coat.secondary.language, "JavaScript");
        assert_eq!(coat.tertiary.language, "C++");
        assert_eq!(coat.primary.base, [0x35, 0x72, 0xA5]);
    }

    #[test]
    fn dominant_language_is_tinted_more_strongly() {
        let heavy = DragonCoat::from_languages(&[lang("Python", "#3572A5", 9)]).expect("coat");
        let mixed = DragonCoat::from_languages(&[
            lang("Python", "#3572A5", 4),
            lang("Go", "#00ADD8", 4),
            lang("Rust", "#dea584", 4),
        ])
        .expect("coat");
        assert!(heavy.primary.strength > mixed.primary.strength);
    }

    #[test]
    fn single_language_fills_all_zones() {
        let coat = DragonCoat::from_languages(&[lang("Go", "#00ADD8", 2)]).expect("coat");
        assert_eq!(coat.secondary.language, "Go");
        assert_eq!(coat.tertiary.language, "Go");
    }

    #[test]
    fn very_dark_colours_are_lifted() {
        let coat = DragonCoat::from_languages(&[lang("Lua", "#000080", 1)]).expect("coat");
        assert!(coat.primary.base.iter().map(|c| u32::from(*c)).sum::<u32>() > 128);
    }
}
