//! Sprite layers: loading, language recolouring, rotation and compositing.
//!
//! Each dragon stage is stored as two same-sized RGBA layers (`*_body.png`, `*_wing.png`).
//! The wing is its own layer so it can swing around the shoulder pivot while the body stays
//! still. The egg is a single layer. All art is compiled into the binary.

use std::f32::consts::PI;

use crate::coat::{DragonCoat, Ramp};
use crate::stage::Stage;

/// Pixels with alpha below this are treated as transparent (GIF only has 1-bit alpha).
pub const ALPHA_CUTOFF: u8 = 128;

/// An RGBA image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// Axis-aligned rectangle in layer pixels (`x1`/`y1` exclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
}

/// Everything needed to draw and animate one stage.
#[derive(Clone, Debug)]
pub struct Art {
    pub body: Layer,
    pub wing: Option<Layer>,
    /// Body pixels that are really the static root of the wing (tinted like the wing).
    pub root: Option<Layer>,
    /// Shoulder pivot the wing rotates around, in layer pixels.
    pub pivot: (f32, f32),
    /// Eye area used for blinking.
    pub eye: Option<Rect>,
    /// Mouth position used as the origin of the fire breath.
    pub mouth: Option<(i32, i32)>,
}

type Png = &'static [u8];

macro_rules! png {
    ($name:literal) => {
        include_bytes!(concat!("../assets/dragons/", $name, ".png")) as Png
    };
}

impl Layer {
    /// Decode an 8-bit RGBA PNG.
    pub fn decode(bytes: &[u8]) -> Result<Layer, String> {
        let decoder = png::Decoder::new(bytes);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0_u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return Err("sprite layers must be 8-bit RGBA PNGs".to_owned());
        }
        buf.truncate(info.buffer_size());
        Ok(Layer {
            w: info.width as usize,
            h: info.height as usize,
            rgba: buf,
        })
    }

    fn px(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.w + x) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

impl Art {
    /// Load the embedded art for `stage`.
    pub fn load(stage: Stage) -> Result<Art, String> {
        let (body, wing, pivot, eye, mouth): (Png, Option<(Png, Png)>, _, _, _) = match stage {
            Stage::Egg => (png!("egg"), None, (0.0, 0.0), None, None),
            Stage::Baby => (
                png!("baby_body"),
                Some((png!("baby_wing"), png!("baby_root"))),
                (82.0, 68.0),
                Some(Rect {
                    x0: 26,
                    y0: 42,
                    x1: 44,
                    y1: 58,
                }),
                None,
            ),
            Stage::Young => (
                png!("young_body"),
                Some((png!("young_wing"), png!("young_root"))),
                (95.0, 95.0),
                Some(Rect {
                    x0: 27,
                    y0: 49,
                    x1: 47,
                    y1: 66,
                }),
                None,
            ),
            Stage::Adolescent => (
                png!("adolescent_body"),
                Some((png!("adolescent_wing"), png!("adolescent_root"))),
                (100.0, 95.0),
                Some(Rect {
                    x0: 30,
                    y0: 47,
                    x1: 48,
                    y1: 65,
                }),
                Some((1, 69)),
            ),
            Stage::Adult => (
                png!("adult_body"),
                Some((png!("adult_wing"), png!("adult_root"))),
                (112.0, 105.0),
                Some(Rect {
                    x0: 32,
                    y0: 61,
                    x1: 59,
                    y1: 81,
                }),
                Some((1, 87)),
            ),
            Stage::Legendary => (
                png!("legendary_body"),
                Some((png!("legendary_wing"), png!("legendary_root"))),
                (138.0, 115.0),
                Some(Rect {
                    x0: 58,
                    y0: 82,
                    x1: 77,
                    y1: 100,
                }),
                Some((34, 113)),
            ),
        };
        let body = Layer::decode(body)?;
        let (wing, root) = match wing {
            Some((w, r)) => (Some(Layer::decode(w)?), Some(Layer::decode(r)?)),
            None => (None, None),
        };
        for extra in wing.iter().chain(root.iter()) {
            if (extra.w, extra.h) != (body.w, body.h) {
                return Err(format!(
                    "{stage:?}: wing/root and body layers differ in size"
                ));
            }
        }
        Ok(Art {
            body,
            wing,
            root,
            pivot,
            eye,
            mouth,
        })
    }
}

// ----------------------------------------------------------------------------------------
// Recolouring
// ----------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Body,
    Wing,
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let (r, g, b) = (r / 255.0, g / 255.0, b / 255.0);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let h = if d < 1e-6 {
        0.0
    } else if (mx - r).abs() < f32::EPSILON {
        ((g - b) / d).rem_euclid(6.0)
    } else if (mx - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, if mx > 0.0 { d / mx } else { 0.0 }, mx)
}

fn sample_ramp(ramp: &Ramp, t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0) * 3.0;
    let i = (t.floor() as usize).min(2);
    let f = t - i as f32;
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        *o = ramp[i][c] * (1.0 - f) + ramp[i + 1][c] * f;
    }
    out
}

fn lerp3(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ]
}

/// Re-tint a layer while keeping its shading: brightness picks the position on the ramp.
///
/// * green/teal/blue pixels -> primary ramp (secondary on the wing layer)
/// * cream pixels in the top third (horns) -> tertiary highlight; elsewhere (belly) -> primary
///   highlight; on the wing layer warm edges follow the wing colour
/// * eyes, whites and the dark outline are left alone
fn recolor(layer: &mut Layer, coat: &DragonCoat, part: Part, root: Option<&Layer>) {
    let horn_limit = 0.30 * layer.h as f32;
    for y in 0..layer.h {
        for x in 0..layer.w {
            let i = (y * layer.w + x) * 4;
            if layer.rgba[i + 3] == 0 {
                continue;
            }
            let orig = [
                f32::from(layer.rgba[i]),
                f32::from(layer.rgba[i + 1]),
                f32::from(layer.rgba[i + 2]),
            ];
            let (h, s, v) = rgb_to_hsv(orig[0], orig[1], orig[2]);
            let luma = (0.299 * orig[0] + 0.587 * orig[1] + 0.114 * orig[2]) / 255.0;
            let t = ((luma - 0.07) / 0.78).clamp(0.0, 1.0);
            let green = (70.0..=260.0).contains(&h) && s >= 0.12 && v >= 0.05;
            let warm = (15.0..70.0).contains(&h) && s >= 0.10 && v >= 0.55;
            let in_root = root.is_some_and(|r| r.rgba[i + 3] != 0);
            let wing = part == Part::Wing || in_root;

            let result = if green || (wing && warm) {
                let tint = if wing { &coat.secondary } else { &coat.primary };
                Some((sample_ramp(&tint.ramp, t), tint.strength))
            } else if warm {
                if (y as f32) < horn_limit {
                    let tint = &coat.tertiary;
                    Some((lerp3(orig, tint.ramp[3], 0.6), tint.strength))
                } else {
                    let tint = &coat.primary;
                    Some((lerp3(orig, tint.ramp[3], 0.4), tint.strength))
                }
            } else {
                None
            };

            if let Some((target, strength)) = result {
                let out = lerp3(orig, target, strength);
                for (dst, v) in layer.rgba[i..i + 3].iter_mut().zip(out) {
                    *dst = v.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

impl Art {
    /// Apply a language coat to every layer.
    pub fn tinted(mut self, coat: &DragonCoat) -> Art {
        let root = self.root.take();
        recolor(&mut self.body, coat, Part::Body, root.as_ref());
        self.root = root;
        if let Some(w) = self.wing.as_mut() {
            recolor(w, coat, Part::Wing, None);
        }
        self
    }
}

// ----------------------------------------------------------------------------------------
// Blinking
// ----------------------------------------------------------------------------------------

/// How far the eyelids are closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lids {
    Open,
    Half,
    Closed,
}

/// Dark outline colour used for eyelid lines.
pub const OUTLINE: [u8; 3] = [18, 32, 30];

/// Return a copy of `body` with the eye covered by skin and an eyelid line drawn.
pub fn with_lids(body: &Layer, eye: Rect, lids: Lids) -> Layer {
    let mut out = body.clone();
    if lids == Lids::Open {
        return out;
    }
    let (w, h) = (eye.x1 - eye.x0, eye.y1 - eye.y0);
    let cy = eye.y0 + h / 2;
    // Skin colour: first clearly-green opaque pixel to the left of the eye.
    let mut skin = None;
    for dx in 2..=8 {
        let x = eye.x0.saturating_sub(dx);
        let p = body.px(x, cy);
        if p[3] >= ALPHA_CUTOFF && p[1] > p[0].saturating_add(12) && p[1] > 70 {
            skin = Some([p[0], p[1], p[2]]);
            break;
        }
    }
    let Some(skin) = skin else {
        return out;
    };
    let cover_to = match lids {
        Lids::Half => eye.y0 + h / 2,
        _ => eye.y1,
    };
    for y in eye.y0..cover_to.min(body.h) {
        for x in eye.x0..eye.x1.min(body.w) {
            let p = body.px(x, y);
            if p[3] < ALPHA_CUTOFF {
                continue;
            }
            let luma = 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
            let near_white = p[0].min(p[1]).min(p[2]) > 200;
            let warm = p[0] > p[1].saturating_add(12);
            if luma < 110.0 || near_white || warm {
                let i = (y * body.w + x) * 4;
                out.rgba[i..i + 3].copy_from_slice(&skin);
            }
        }
    }
    // Eyelid line.
    let line_y = match lids {
        Lids::Half => eye.y0 + h / 2,
        _ => eye.y0 + (h * 6) / 10,
    };
    let thickness = if h >= 16 { 2 } else { 1 };
    for dy in 0..thickness {
        let y = line_y + dy;
        if y >= body.h {
            continue;
        }
        for x in (eye.x0 + 1)..(eye.x1.saturating_sub(w / 8 + 1)) {
            let i = (y * body.w + x) * 4;
            out.rgba[i..i + 3].copy_from_slice(&OUTLINE);
            out.rgba[i + 3] = 255;
        }
    }
    out
}

// ----------------------------------------------------------------------------------------
// Canvas
// ----------------------------------------------------------------------------------------

/// A fixed-size RGBA drawing surface with binary alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            rgba: vec![0; w * h * 4],
        }
    }

    /// Set one pixel (out-of-bounds writes are ignored).
    pub fn put(&mut self, x: i32, y: i32, rgb: [u8; 3]) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        self.rgba[i..i + 3].copy_from_slice(&rgb);
        self.rgba[i + 3] = 255;
    }

    /// Fill a `size`x`size` block whose top-left corner is `(x, y)`.
    pub fn block(&mut self, x: i32, y: i32, size: i32, rgb: [u8; 3]) {
        for dy in 0..size {
            for dx in 0..size {
                self.put(x + dx, y + dy, rgb);
            }
        }
    }

    /// Alpha-blend one pixel over the canvas (standard "over" compositing);
    /// out-of-bounds writes are ignored.
    pub fn put_blend(&mut self, x: i32, y: i32, rgb: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        let dst_a = self.rgba[i + 3] as f32 / 255.0;
        let out_a = alpha + dst_a * (1.0 - alpha);
        if out_a <= 0.0 {
            return;
        }
        let dst_w = dst_a * (1.0 - alpha);
        for (c, &channel) in rgb.iter().enumerate() {
            let blended = (channel as f32 * alpha + self.rgba[i + c] as f32 * dst_w) / out_a;
            self.rgba[i + c] = blended.round() as u8;
        }
        self.rgba[i + 3] = (out_a * 255.0).round() as u8;
    }

    /// Alpha-blend a `size`x`size` block (the fire breath halo).
    pub fn block_soft(&mut self, x: i32, y: i32, size: i32, rgb: [u8; 3], alpha: f32) {
        for dy in 0..size {
            for dx in 0..size {
                self.put_blend(x + dx, y + dy, rgb, alpha);
            }
        }
    }

    /// Draw `layer` with its top-left at `origin`.
    pub fn blit(&mut self, layer: &Layer, origin: (i32, i32)) {
        for sy in 0..layer.h {
            for sx in 0..layer.w {
                let p = layer.px(sx, sy);
                if p[3] >= ALPHA_CUTOFF {
                    self.put(
                        origin.0 + sx as i32,
                        origin.1 + sy as i32,
                        [p[0], p[1], p[2]],
                    );
                }
            }
        }
    }

    /// Draw `layer` rotated clockwise by `deg` and scaled about `pivot` (layer pixels).
    /// Nearest-neighbour sampling keeps pixel art crisp.
    pub fn blit_rotated(
        &mut self,
        layer: &Layer,
        origin: (i32, i32),
        pivot: (f32, f32),
        deg: f32,
        scale: f32,
    ) {
        let (sn, c) = (deg * PI / 180.0).sin_cos();
        let pcx = origin.0 as f32 + pivot.0;
        let pcy = origin.1 as f32 + pivot.1;
        // Destination bounds from the transformed layer corners.
        let corners = [
            (0.0, 0.0),
            (layer.w as f32, 0.0),
            (0.0, layer.h as f32),
            (layer.w as f32, layer.h as f32),
        ];
        let (mut x_min, mut y_min, mut x_max, mut y_max) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (cx, cy) in corners {
            let dx = cx - pivot.0;
            let dy = cy - pivot.1;
            let x = pcx + scale * (c * dx - sn * dy);
            let y = pcy + scale * (sn * dx + c * dy);
            x_min = x_min.min(x);
            y_min = y_min.min(y);
            x_max = x_max.max(x);
            y_max = y_max.max(y);
        }
        let x0 = (x_min.floor() as i32 - 2).max(0);
        let y0 = (y_min.floor() as i32 - 2).max(0);
        let x1 = (x_max.ceil() as i32 + 2).min(self.w as i32);
        let y1 = (y_max.ceil() as i32 + 2).min(self.h as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - pcx;
                let dy = y as f32 + 0.5 - pcy;
                let sx = ((c * dx + sn * dy) / scale + pivot.0).floor();
                let sy = ((-sn * dx + c * dy) / scale + pivot.1).floor();
                if sx < 0.0 || sy < 0.0 || sx >= layer.w as f32 || sy >= layer.h as f32 {
                    continue;
                }
                let p = layer.px(sx as usize, sy as usize);
                if p[3] >= ALPHA_CUTOFF {
                    self.put(x, y, [p[0], p[1], p[2]]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_stage_art_loads_with_matching_layers() {
        for stage in Stage::ALL {
            let art = Art::load(stage).expect("art");
            assert_eq!(art.wing.is_some(), stage != Stage::Egg, "{stage:?}");
            if let Some(wing) = &art.wing {
                assert_eq!((wing.w, wing.h), (art.body.w, art.body.h));
                assert!(art.pivot.0 < art.body.w as f32 && art.pivot.1 < art.body.h as f32);
            }
            if let Some(eye) = art.eye {
                assert!(
                    eye.x1 <= art.body.w && eye.y1 <= art.body.h,
                    "{stage:?} eye rect"
                );
            }
        }
    }

    #[test]
    fn rotation_by_zero_is_identity() {
        let art = Art::load(Stage::Adult).expect("art");
        let wing = art.wing.expect("wing");
        let mut a = Canvas::new(400, 300);
        let mut b = Canvas::new(400, 300);
        a.blit(&wing, (20, 20));
        b.blit_rotated(&wing, (20, 20), art.pivot, 0.0, 1.0);
        assert!(a == b, "zero rotation must equal a plain blit");
    }

    #[test]
    fn blink_changes_only_the_eye_area() {
        let art = Art::load(Stage::Adult).expect("art");
        let eye = art.eye.expect("eye");
        let closed = with_lids(&art.body, eye, Lids::Closed);
        assert!(closed != art.body);
        for y in 0..art.body.h {
            for x in 0..art.body.w {
                let inside = x >= eye.x0 && x < eye.x1 && y >= eye.y0 && y < eye.y1 + 3;
                if !inside {
                    assert_eq!(
                        closed.px(x, y),
                        art.body.px(x, y),
                        "pixel {x},{y} outside eye changed"
                    );
                }
            }
        }
        assert!(with_lids(&art.body, eye, Lids::Open) == art.body);
    }
}
