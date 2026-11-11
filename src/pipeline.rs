//! End-to-end rendering: snapshot in, files out.

use std::fs;
use std::path::{Path, PathBuf};

use crate::coat::DragonCoat;
use crate::history::AccountSnapshot;
use crate::output::{encode_gif, encode_png, preview_html, share_card};
use crate::render::{render_loop, render_still};
use crate::stage::Stage;

/// What a render produced.
#[derive(Debug, Clone)]
pub struct RenderReport {
    pub stage: Stage,
    pub contributions: u64,
    /// Languages that tinted the dragon (empty = canonical emerald dragon).
    pub tinted_by: Vec<String>,
    pub gif: PathBuf,
    pub png: PathBuf,
    pub share_card: PathBuf,
    pub preview: PathBuf,
}

/// Contribution count that decides the stage.
pub fn contributions_of(snapshot: &AccountSnapshot) -> u64 {
    snapshot
        .total_contributions
        .max(snapshot.monthly_contributions)
}

/// Render `snapshot` into `output_dir`, writing `dragon.gif`, `dragon.png`,
/// `share-card.png` and `preview.html`.
///
/// With `canon` the language colours are ignored and the original emerald dragon is drawn.
pub fn render_snapshot(
    snapshot: &AccountSnapshot,
    output_dir: &Path,
    canon: bool,
) -> Result<RenderReport, String> {
    let contributions = contributions_of(snapshot);
    let stage = Stage::from_contributions(contributions);
    let coat = DragonCoat::from_snapshot(snapshot, canon);

    let frames = render_loop(stage, coat.as_ref())?;
    let still = render_still(stage, coat.as_ref())?;

    fs::create_dir_all(output_dir).map_err(|e| format!("{}: {e}", output_dir.display()))?;
    let write = |name: &str, bytes: &[u8]| -> Result<PathBuf, String> {
        let path = output_dir.join(name);
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    };

    let gif = write("dragon.gif", &encode_gif(&frames)?)?;
    let png = write("dragon.png", &encode_png(&still)?)?;
    let card = write("share-card.png", &encode_png(&share_card(&still))?)?;
    let caption = format!(
        "@{} - {} ({} contributions)",
        snapshot.login, stage, contributions
    );
    let preview = write("preview.html", preview_html(&caption).as_bytes())?;

    let tinted_by = coat
        .map(|c| {
            let mut names = vec![
                c.primary.language,
                c.secondary.language,
                c.tertiary.language,
            ];
            names.dedup();
            names
        })
        .unwrap_or_default();

    Ok(RenderReport {
        stage,
        contributions,
        tinted_by,
        gif,
        png,
        share_card: card,
        preview,
    })
}
