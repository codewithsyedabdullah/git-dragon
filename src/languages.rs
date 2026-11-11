//! Offline GitHub Linguist language colours.
//!
//! Live runs use the colour GitHub returns for each language. This table is only the
//! fallback for the tokenless REST path and for the `--languages` demo flag. Unknown
//! languages get a stable colour derived from their name, so two different languages never
//! collapse onto the same colour.

/// Linguist colours for the most common languages (`name`, `#rrggbb`).
const LINGUIST: &[(&str, &str)] = &[
    ("Assembly", "#6E4C13"),
    ("C", "#555555"),
    ("C#", "#178600"),
    ("C++", "#f34b7d"),
    ("CSS", "#663399"),
    ("Clojure", "#db5855"),
    ("Dart", "#00B4AB"),
    ("Dockerfile", "#384d54"),
    ("Elixir", "#6e4a7e"),
    ("Erlang", "#B83998"),
    ("F#", "#b845fc"),
    ("Go", "#00ADD8"),
    ("Groovy", "#e69f56"),
    ("HTML", "#e34c26"),
    ("Haskell", "#5e5086"),
    ("Java", "#b07219"),
    ("JavaScript", "#f1e05a"),
    ("Julia", "#a270ba"),
    ("Jupyter Notebook", "#DA5B0B"),
    ("Kotlin", "#A97BFF"),
    ("Lua", "#000080"),
    ("MATLAB", "#e16737"),
    ("Makefile", "#427819"),
    ("Nix", "#7e7eff"),
    ("OCaml", "#ef7a08"),
    ("Objective-C", "#438eff"),
    ("PHP", "#4F5D95"),
    ("Perl", "#0298c3"),
    ("PowerShell", "#012456"),
    ("Python", "#3572A5"),
    ("R", "#198CE7"),
    ("Ruby", "#701516"),
    ("Rust", "#dea584"),
    ("SCSS", "#c6538c"),
    ("Scala", "#c22d40"),
    ("Shell", "#89e051"),
    ("Svelte", "#ff3e00"),
    ("Swift", "#F05138"),
    ("TeX", "#3D6117"),
    ("TypeScript", "#3178c6"),
    ("Vue", "#41b883"),
    ("Zig", "#ec915c"),
];

/// Hex colour for a language name (case-insensitive), `#rrggbb`.
pub fn language_hex(name: &str) -> String {
    let wanted = name.trim();
    if let Some((_, hex)) = LINGUIST
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(wanted))
    {
        return (*hex).to_owned();
    }
    hashed_hex(wanted)
}

/// Deterministic, pleasant colour for an unknown language (FNV-1a hash -> HSV).
fn hashed_hex(name: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.to_ascii_lowercase().bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    let hue = (hash % 360) as f32;
    let [r, g, b] = hsv_to_rgb(hue, 0.55, 0.85);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_languages_use_linguist_colours() {
        assert_eq!(language_hex("Python"), "#3572A5");
        assert_eq!(language_hex("javascript"), "#f1e05a");
        assert_eq!(language_hex("Rust"), "#dea584");
    }

    #[test]
    fn unknown_languages_are_stable_and_distinct() {
        let a = language_hex("Frobnicate");
        assert_eq!(a, language_hex("frobnicate"));
        assert_ne!(a, language_hex("Quuxlang"));
        assert_eq!(a.len(), 7);
    }
}
