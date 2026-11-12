//! Procedural pixel effects drawn on top of the sprites: fire breath, magical embers,
//! twinkling stars and the egg's glint. Every effect is a pure function of the frame index,
//! so renders are reproducible and loops are seamless.

use std::f32::consts::TAU;

use crate::sprite::Canvas;
use crate::stage::Stage;

/// Flame colours from hottest to coolest.
const FIRE_PALE: [u8; 3] = [255, 244, 170];
const FIRE_YELLOW: [u8; 3] = [255, 200, 40];
const FIRE_ORANGE: [u8; 3] = [255, 130, 20];
const FIRE_RED: [u8; 3] = [215, 60, 10];
const FIRE_EMBER: [u8; 3] = [140, 28, 8];

/// Edge length of one chunky effect "pixel".
const BLOCK: i32 = 2;

/// Soft warm covering around the flame: halo rings in block units with a
/// blend alpha, hottest ring first. Mirrored in assets/js/dragon.js.
const FIRE_HALO: [(i32, i32, f32); 16] = [
    (-1, 0, 0.40),
    (1, 0, 0.40),
    (0, -1, 0.40),
    (0, 1, 0.40),
    (-1, -1, 0.22),
    (1, -1, 0.22),
    (-1, 1, 0.22),
    (1, 1, 0.22),
    (-2, 0, 0.14),
    (2, 0, 0.14),
    (0, -2, 0.14),
    (0, 2, 0.14),
    (-2, -2, 0.07),
    (2, -2, 0.07),
    (-2, 2, 0.07),
    (2, 2, 0.07),
];
/// How far the halo colour is dimmed towards black (a covering, not a flame).
const GLOW_DIM: f32 = 0.45;

/// Size of a fire breath.
#[derive(Clone, Copy, Debug)]
pub struct FireSpec {
    /// Maximum length of the flame in pixels.
    pub length: f32,
    /// Maximum half-width of the flame in pixels.
    pub radius: f32,
}

/// Fire breath of a stage (`None` for stages too young to breathe fire).
pub fn fire_spec(stage: Stage) -> Option<FireSpec> {
    match stage {
        Stage::Adolescent => Some(FireSpec {
            length: 80.0,
            radius: 14.0,
        }),
        Stage::Adult => Some(FireSpec {
            length: 115.0,
            radius: 18.0,
        }),
        Stage::Legendary => Some(FireSpec {
            length: 140.0,
            radius: 22.0,
        }),
        _ => None,
    }
}

/// Small deterministic hash -> 0.0..1.0.
fn noise(a: i32, b: i32, c: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1)
        ^ (b as u32).wrapping_mul(0x85eb_ca6b)
        ^ (c as u32).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65_535.0
}

/// Draw a flame leaving the mouth to the left.
///
/// `progress` runs 0.0..1.0 over the breath; `tick` is the frame index (drives flicker).
/// A soft warm covering (dim halo rings) is blended around the flame sheet first,
/// then the hot core is drawn opaque on top.
pub fn draw_fire(
    canvas: &mut Canvas,
    spec: FireSpec,
    mouth: (i32, i32),
    progress: f32,
    tick: usize,
) {
    let envelope = (progress.clamp(0.0, 1.0) * std::f32::consts::PI)
        .sin()
        .powf(0.6);
    let length = spec.length * envelope;
    if length < 8.0 {
        return;
    }
    let tick_i = tick as i32;
    let steps = (length / BLOCK as f32) as i32;
    let mut blocks: Vec<(i32, i32, [u8; 3])> = Vec::new();
    for i in 0..steps {
        let d = (i * BLOCK) as f32;
        let f = d / length;
        let half = spec.radius * envelope * (0.30 + 0.70 * f.powf(0.8));
        let centre = -d * 0.10 + 3.0 * (d * 0.11 - tick as f32 * 0.55).sin();
        let rows = (half / BLOCK as f32).ceil() as i32;
        for j in -rows..=rows {
            let dy = (j * BLOCK) as f32;
            let n = if half > 0.0 { dy.abs() / half } else { 1.0 };
            let jitter = noise(i, j, tick_i / 2);
            let heat = 1.0 - 0.75 * f - 0.55 * n + 0.35 * (jitter - 0.5);
            if heat < 0.08 {
                continue;
            }
            let rgb = if heat > 0.75 {
                FIRE_PALE
            } else if heat > 0.5 {
                FIRE_YELLOW
            } else if heat > 0.3 {
                FIRE_ORANGE
            } else if heat > 0.15 {
                FIRE_RED
            } else {
                FIRE_EMBER
            };
            let x = mouth.0 - i * BLOCK - BLOCK;
            let y = mouth.1 + (centre + dy) as i32 - BLOCK / 2;
            blocks.push((x, y, rgb));
        }
    }

    // Soft warm covering around the flame sheet, then the hot core on top.
    for &(x, y, rgb) in &blocks {
        let glow = [
            (rgb[0] as f32 * GLOW_DIM).round() as u8,
            (rgb[1] as f32 * GLOW_DIM).round() as u8,
            (rgb[2] as f32 * GLOW_DIM).round() as u8,
        ];
        for &(ox, oy, a) in FIRE_HALO.iter() {
            canvas.block_soft(x + ox * BLOCK, y + oy * BLOCK, BLOCK, glow, a);
        }
    }
    for &(x, y, rgb) in &blocks {
        canvas.block(x, y, BLOCK, rgb);
    }
}

/// A `+` shaped twinkle.
fn star(canvas: &mut Canvas, x: i32, y: i32, arm: i32, colour: [u8; 3]) {
    canvas.block(x - 1, y - 1, BLOCK, [255, 255, 255]);
    for k in 1..=arm {
        let o = k * BLOCK;
        canvas.block(x - 1 + o, y - 1, BLOCK, colour);
        canvas.block(x - 1 - o, y - 1, BLOCK, colour);
        canvas.block(x - 1, y - 1 + o, BLOCK, colour);
        canvas.block(x - 1, y - 1 - o, BLOCK, colour);
    }
}

/// Colours of the legendary magic.
#[derive(Clone, Copy, Debug)]
pub struct MagicColours {
    pub ember: [u8; 3],
    pub spark: [u8; 3],
}

impl Default for MagicColours {
    fn default() -> Self {
        Self {
            ember: [96, 240, 214],
            spark: [255, 226, 120],
        }
    }
}

/// Rising embers and twinkling stars around the legendary dragon.
///
/// `area` is the sprite rectangle `(x, y, w, h)` on the canvas.
pub fn draw_magic(
    canvas: &mut Canvas,
    area: (i32, i32, i32, i32),
    tick: usize,
    frames: usize,
    colours: MagicColours,
) {
    let phase = tick as f32 / frames as f32;
    let (ax, ay, aw, ah) = area;

    // Embers rise twice per loop and sway once.
    let span = (ah as f32 * 0.9).max(1.0);
    for k in 0..18_i32 {
        let bx = ax as f32 + 10.0 + noise(k, 1, 7) * (aw as f32 - 20.0);
        let start = noise(k, 2, 7) * span;
        let lift = (start - phase * span * 2.0).rem_euclid(span);
        let y = ay as f32 + ah as f32 - 6.0 - lift;
        let x = bx + 3.0 * (TAU * (phase + noise(k, 3, 7))).sin();
        let fade = lift / span; // 0 near the ground, 1 at the top
        if fade > 0.92 {
            continue;
        }
        let size = if fade > 0.6 { 1 } else { BLOCK };
        let rgb = if k % 3 == 0 {
            [210, 255, 245]
        } else {
            colours.ember
        };
        canvas.block(x as i32, y as i32, size, rgb);
    }

    // Stars twinkle at fixed spots.
    for k in 0..6_i32 {
        let sx = ax + 8 + (noise(k, 4, 9) * (aw - 16) as f32) as i32;
        let sy = ay + 4 + (noise(k, 5, 9) * (ah as f32 * 0.55)) as i32;
        let s = (TAU * (phase * 2.0 + noise(k, 6, 9))).sin();
        if s > 0.25 {
            let arm = 1 + (s * 3.0) as i32;
            star(canvas, sx, sy, arm, colours.spark);
        }
    }
}

/// A slow glint on the egg shell.
pub fn draw_egg_glint(canvas: &mut Canvas, origin: (i32, i32), tick: usize, frames: usize) {
    let s = (TAU * tick as f32 / frames as f32).sin();
    if s > 0.2 {
        let arm = 1 + (s * 2.0) as i32;
        star(canvas, origin.0 + 26, origin.1 + 30, arm, [255, 250, 215]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(c: &Canvas) -> usize {
        c.rgba.chunks(4).filter(|p| p[3] != 0).count()
    }

    #[test]
    fn only_older_stages_breathe_fire() {
        for stage in Stage::ALL {
            assert_eq!(
                fire_spec(stage).is_some(),
                stage.breathes_fire(),
                "{stage:?}"
            );
        }
    }

    #[test]
    fn fire_grows_and_fades() {
        let spec = fire_spec(Stage::Adult).expect("fire");
        let mut none = Canvas::new(400, 200);
        draw_fire(&mut none, spec, (300, 100), 0.0, 3);
        assert_eq!(lit(&none), 0);
        let mut peak = Canvas::new(400, 200);
        draw_fire(&mut peak, spec, (300, 100), 0.5, 3);
        assert!(lit(&peak) > 200);
    }

    #[test]
    fn fire_has_soft_covering() {
        let spec = fire_spec(Stage::Legendary).expect("fire");
        let mut canvas = Canvas::new(400, 200);
        draw_fire(&mut canvas, spec, (300, 100), 0.5, 3);
        // The halo adds dim warm pixels the hot core never writes.
        let dim_warm = canvas
            .rgba
            .chunks(4)
            .any(|p| p[3] != 0 && p[0] > 0 && p[0] < 120 && p[1] < 90 && p[2] < 90 && p[1] > p[2]);
        assert!(dim_warm, "halo should add dim warm pixels around the flame");
    }

    #[test]
    fn magic_is_periodic() {
        let area = (50, 40, 200, 180);
        let mut a = Canvas::new(400, 300);
        let mut b = Canvas::new(400, 300);
        draw_magic(&mut a, area, 0, 64, MagicColours::default());
        draw_magic(&mut b, area, 64, 64, MagicColours::default());
        assert!(a == b, "magic must be periodic");
        assert!(lit(&a) > 0);
    }
}
