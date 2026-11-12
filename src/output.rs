//! Encoders: animated GIF, static PNG, social share card and the HTML preview page.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::render::{Frame, CANVAS_H, CANVAS_W, FRAME_DELAY_CS};
use crate::sprite::Canvas;

/// NeuQuant sampling factor for GIF palettes (1 = best quality, 30 = fastest).
const GIF_QUALITY: i32 = 3;
/// Palette slot 0 is reserved for transparency.
const TRANSPARENT_INDEX: u8 = 0;
/// Palette entries available for colours (256 minus the transparent slot).
const MAX_COLOURS: usize = 255;

/// Share card size: GitHub's recommended social preview dimensions.
pub const SHARE_CARD_W: usize = 1280;
pub const SHARE_CARD_H: usize = 640;
const SHARE_CARD_BG: [u8; 3] = [13, 17, 23];

/// Bounding box `(left, top, width, height)` of the visible pixels.
fn drawn_box(frame: &Frame) -> (usize, usize, usize, usize) {
    let (mut left, mut top, mut right, mut bottom) = (frame.w, frame.h, 0_usize, 0_usize);
    for y in 0..frame.h {
        for x in 0..frame.w {
            if frame.rgba[(y * frame.w + x) * 4 + 3] != 0 {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    if left > right {
        (0, 0, 1, 1)
    } else {
        (left, top, right + 1 - left, bottom + 1 - top)
    }
}

/// One palette shared by every frame: static pixels then get identical indices in every
/// frame, which compresses far better and keeps colours from shimmering between frames.
///
/// Uses the exact colours when there are at most 255 of them, otherwise a NeuQuant palette
/// trained on a spread of frames.
struct Palette {
    /// `r,g,b` triples; entry 0 is the transparent slot.
    table: Vec<u8>,
    exact: HashMap<[u8; 3], u8>,
    quant: Option<color_quant::NeuQuant>,
    cache: HashMap<[u8; 3], u8>,
}

impl Palette {
    fn build(frames: &[Frame]) -> Palette {
        let mut unique: HashSet<[u8; 3]> = HashSet::new();
        for frame in frames {
            for p in frame.rgba.as_chunks::<4>().0.iter().filter(|p| p[3] != 0) {
                unique.insert([p[0], p[1], p[2]]);
            }
        }
        if unique.len() <= MAX_COLOURS {
            let mut colours: Vec<[u8; 3]> = unique.into_iter().collect();
            colours.sort_unstable();
            let mut table = vec![0, 0, 0];
            let mut exact = HashMap::new();
            for (i, c) in colours.iter().enumerate() {
                table.extend_from_slice(c);
                exact.insert(*c, (i + 1) as u8);
            }
            return Palette {
                table,
                exact,
                quant: None,
                cache: HashMap::new(),
            };
        }
        // Train on a spread of frames so rare colours (fire, sparks) are represented.
        let mut training: Vec<u8> = Vec::new();
        let step = (frames.len() / 8).max(1);
        for frame in frames.iter().step_by(step) {
            for p in frame.rgba.as_chunks::<4>().0.iter().filter(|p| p[3] != 0) {
                training.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        let quant = color_quant::NeuQuant::new(GIF_QUALITY, MAX_COLOURS, &training);
        let mut table = vec![0, 0, 0];
        table.extend_from_slice(&quant.color_map_rgb());
        Palette {
            table,
            exact: HashMap::new(),
            quant: Some(quant),
            cache: HashMap::new(),
        }
    }

    fn index(&mut self, rgb: [u8; 3]) -> u8 {
        if let Some(&i) = self.exact.get(&rgb) {
            return i;
        }
        if let Some(&i) = self.cache.get(&rgb) {
            return i;
        }
        let i = self
            .quant
            .as_ref()
            .map_or(1, |q| q.index_of(&[rgb[0], rgb[1], rgb[2], 255]) as u8 + 1);
        self.cache.insert(rgb, i);
        i
    }
}

/// Encode frames as an endlessly looping, transparent GIF.
pub fn encode_gif(frames: &[Frame]) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("a GIF needs at least one frame".to_owned());
    }
    if frames.iter().any(|f| (f.w, f.h) != (CANVAS_W, CANVAS_H)) {
        return Err("all frames must be exactly CANVAS_W x CANVAS_H".to_owned());
    }
    let mut palette = Palette::build(frames);
    let mut bytes = Vec::new();
    {
        let mut encoder =
            gif::Encoder::new(&mut bytes, CANVAS_W as u16, CANVAS_H as u16, &palette.table)
                .map_err(|e| e.to_string())?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;
        for frame in frames {
            let (left, top, w, h) = drawn_box(frame);
            let mut indices = Vec::with_capacity(w * h);
            for y in top..top + h {
                for x in left..left + w {
                    let i = (y * frame.w + x) * 4;
                    indices.push(if frame.rgba[i + 3] == 0 {
                        TRANSPARENT_INDEX
                    } else {
                        palette.index([frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]])
                    });
                }
            }
            let gif_frame = gif::Frame {
                left: left as u16,
                top: top as u16,
                width: w as u16,
                height: h as u16,
                delay: FRAME_DELAY_CS,
                dispose: gif::DisposalMethod::Background,
                transparent: Some(TRANSPARENT_INDEX),
                buffer: Cow::Owned(indices),
                ..gif::Frame::default()
            };
            encoder.write_frame(&gif_frame).map_err(|e| e.to_string())?;
        }
    }
    Ok(bytes)
}

/// Encode a canvas as an RGBA PNG.
pub fn encode_png(canvas: &Canvas) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, canvas.w as u32, canvas.h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&canvas.rgba)
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}

/// Place the still on a dark 1280x640 card, scaled 2x with hard pixel edges.
pub fn share_card(still: &Frame) -> Canvas {
    let mut card = Canvas::new(SHARE_CARD_W, SHARE_CARD_H);
    for y in 0..SHARE_CARD_H {
        for x in 0..SHARE_CARD_W {
            card.put(x as i32, y as i32, SHARE_CARD_BG);
        }
    }
    let scale = 2;
    let off_x = (SHARE_CARD_W - still.w * scale) / 2;
    let off_y = (SHARE_CARD_H - still.h * scale) / 2;
    for y in 0..still.h {
        for x in 0..still.w {
            let i = (y * still.w + x) * 4;
            if still.rgba[i + 3] == 0 {
                continue;
            }
            let rgb = [still.rgba[i], still.rgba[i + 1], still.rgba[i + 2]];
            card.block(
                (off_x + x * scale) as i32,
                (off_y + y * scale) as i32,
                scale as i32,
                rgb,
            );
        }
    }
    card
}

/// HTML page that shows the GIF and PNG on GitHub's light and dark backgrounds.
pub fn preview_html(caption: &str) -> String {
    let escaped = caption
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let w = CANVAS_W;
    let h = CANVAS_H;
    let block = |class: &str, title: &str| {
        format!(
            r#"<section class="{class}">
<h2>{title}</h2>
<div class="images">
<figure><img src="dragon.gif" width="{w}" height="{h}" alt="Animated dragon"><figcaption>dragon.gif (animated)</figcaption></figure>
<figure><img src="dragon.png" width="{w}" height="{h}" alt="Dragon still"><figcaption>dragon.png (still)</figcaption></figure>
</div>
</section>
"#
        )
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>git-dragon preview</title>
<style>
  body {{ margin: 0; font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; }}
  h1, h2 {{ margin: 0; font-size: 14px; font-weight: 600; }}
  header, section {{ display: grid; gap: 12px; padding: 16px; }}
  .light {{ background: #ffffff; color: #1f2328; }}
  .dark {{ background: #0d1117; color: #f0f6fc; }}
  .images {{ display: flex; flex-wrap: wrap; gap: 16px; }}
  figure {{ margin: 0; }}
  img {{ display: block; max-width: 100%; height: auto; image-rendering: pixelated; }}
</style>
</head>
<body>
<header class="dark"><h1>{escaped}</h1></header>
{dark}{light}</body>
</html>
"#,
        dark = block("dark", "GitHub dark mode"),
        light = block("light", "GitHub light mode"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{render_loop, render_still, LOOP_FRAMES};
    use crate::stage::Stage;

    #[test]
    fn gif_is_animated_and_loops_forever() {
        let frames = render_loop(Stage::Adult, None).expect("frames");
        let bytes = encode_gif(&frames).expect("gif");
        assert_eq!(&bytes[..6], b"GIF89a");

        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options.read_info(bytes.as_slice()).expect("decode");
        assert_eq!(
            (decoder.width() as usize, decoder.height() as usize),
            (CANVAS_W, CANVAS_H)
        );
        let mut count = 0;
        while let Some(frame) = decoder.read_next_frame().expect("frame") {
            assert_eq!(frame.delay, FRAME_DELAY_CS);
            count += 1;
        }
        assert_eq!(count, LOOP_FRAMES);
    }

    #[test]
    fn png_and_share_card_have_expected_dimensions() {
        let still = render_still(Stage::Baby, None).expect("still");
        let png_bytes = encode_png(&still).expect("png");
        assert_eq!(&png_bytes[1..4], b"PNG");
        let card = share_card(&still);
        assert_eq!((card.w, card.h), (SHARE_CARD_W, SHARE_CARD_H));
        assert!(encode_png(&card).is_ok());
    }

    #[test]
    fn preview_escapes_the_caption() {
        let html = preview_html("<b>&</b>");
        assert!(html.contains("&lt;b&gt;&amp;&lt;/b&gt;"));
        assert!(html.contains("dragon.gif"));
    }

    #[test]
    fn empty_gif_is_rejected() {
        assert!(encode_gif(&[]).is_err());
    }
}
