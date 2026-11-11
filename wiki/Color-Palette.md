# Colour System

## Default (canonical) dragon

Emerald and forest-green scales, turquoise and cyan highlights, cream belly, ivory/golden horns, dark navy-green outline. Used when there is no language data, when no language colour parses, or with `--canon`.

## Language tinting

`DragonCoat::from_languages` takes the top three languages (most used first):

| Zone | Language | Applies to |
|---|---|---|
| primary | 1st | scales, belly, highlights, wing-root |
| secondary | 2nd | wing membranes |
| tertiary | 3rd | horns and crest |

Each zone carries a 4-step ramp (shadow, mid-dark, base, highlight) built from the language colour, and a **strength** between 0 and 1 that grows with the language's share of the account (primary 0.55 - 0.95, secondary 0.45 - 0.95, tertiary 0.50 - 0.95). Pixels are tinted by brightness, so the original shading survives; strength blends the result with the original emerald art. Eyes, outlines and fire are never tinted. With fewer than three languages the last one fills the remaining zones.

Colours come from the GitHub API (Linguist); `src/languages.rs` holds an offline Linguist table (Python `#3572A5`, JavaScript `#f1e05a`, TypeScript `#3178c6`, Rust `#dea584`, Go `#00ADD8` and more). Unknown languages get a stable colour derived from their name. Colours darker than a minimum brightness are lightened so outlines stay visible.

Fire uses a fixed flame palette: pale yellow, yellow, orange, red, ember.
