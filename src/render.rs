//! Frame composition: turns a stage (plus an optional language coat) into animation frames.
//!
//! Timeline of the 48-frame loop (about 3.4 s):
//!
//! * the wing flaps three times (amplitude grows with the stage);
//! * the dragon blinks twice (frames 4-7 and 21-23);
//! * adolescent, adult and legendary dragons breathe fire once (frames 26-45);
//! * the legendary dragon is surrounded by rising embers and twinkling stars;
//! * the egg catches a slow glint.
//!
//! Nothing is random, so the same input always produces byte-identical frames.

use std::f32::consts::TAU;

use crate::coat::DragonCoat;
use crate::effects::{self, MagicColours};
use crate::sprite::{with_lids, Art, Canvas, Layer, Lids};
use crate::stage::Stage;

/// One rendered frame.
pub type Frame = Canvas;

/// Canvas size in pixels. Fixed for every stage so growth is visible when embedded.
pub const CANVAS_W: usize = 640;
pub const CANVAS_H: usize = 300;
/// Frames in one seamless loop.
pub const LOOP_FRAMES: usize = 48;
/// Delay between frames in hundredths of a second (about 14 FPS).
pub const FRAME_DELAY_CS: u16 = 7;
/// Full wing flaps per loop. Must be an integer for the loop to be seamless.
pub const FLAPS_PER_LOOP: usize = 3;
/// Gap between the feet and the bottom edge of the canvas.
const GROUND_MARGIN: usize = 8;

const FIRE_START: usize = 26;
const FIRE_END: usize = 46;

/// Wing swing around the resting angle, in degrees.
fn wing_amplitude(stage: Stage) -> f32 {
    match stage {
        Stage::Egg => 0.0,
        Stage::Baby => 14.0,
        Stage::Young => 16.0,
        Stage::Adolescent => 18.0,
        Stage::Adult => 20.0,
        Stage::Legendary => 22.0,
    }
}

/// Eyelid state at frame `t`.
pub fn lids_at(t: usize) -> Lids {
    match t % LOOP_FRAMES {
        5 | 6 | 22 => Lids::Closed,
        4 | 7 | 21 | 23 => Lids::Half,
        _ => Lids::Open,
    }
}

/// Everything needed to draw any frame of one dragon.
pub struct Rig {
    stage: Stage,
    art: Art,
    half: Option<Layer>,
    closed: Option<Layer>,
    magic: MagicColours,
}

impl Rig {
    /// Build a rig. `coat = None` draws the canonical emerald dragon.
    pub fn new(stage: Stage, coat: Option<&DragonCoat>) -> Result<Rig, String> {
        let mut art = Art::load(stage)?;
        let mut magic = MagicColours::default();
        if let Some(coat) = coat {
            art = art.tinted(coat);
            let ember = coat.secondary.ramp[3].map(|c| c.round().clamp(0.0, 255.0) as u8);
            let spark = coat.tertiary.ramp[3].map(|c| c.round().clamp(0.0, 255.0) as u8);
            magic = MagicColours { ember, spark };
        }
        let (half, closed) = match art.eye {
            Some(eye) => (
                Some(with_lids(&art.body, eye, Lids::Half)),
                Some(with_lids(&art.body, eye, Lids::Closed)),
            ),
            None => (None, None),
        };
        Ok(Rig {
            stage,
            art,
            half,
            closed,
            magic,
        })
    }

    /// Top-left corner of the sprite on the canvas.
    fn origin(&self) -> (i32, i32) {
        let x = (CANVAS_W as i32 - self.art.body.w as i32) / 2;
        let y = CANVAS_H as i32 - self.art.body.h as i32 - GROUND_MARGIN as i32;
        (x, y)
    }

    /// Render frame `t` (any value; it wraps around the loop).
    pub fn frame(&self, t: usize) -> Frame {
        let t = t % LOOP_FRAMES;
        let mut canvas = Canvas::new(CANVAS_W, CANVAS_H);
        let origin = self.origin();

        if let Some(wing) = &self.art.wing {
            let ph = TAU * FLAPS_PER_LOOP as f32 * t as f32 / LOOP_FRAMES as f32;
            let angle = 8.0 + wing_amplitude(self.stage) * ph.sin();
            // The wing foreshortens a little on the downstroke.
            let scale = 1.0 - 0.07 * ph.sin().max(0.0);
            canvas.blit_rotated(wing, origin, self.art.pivot, angle, scale);
        }

        let body = match lids_at(t) {
            Lids::Open => &self.art.body,
            Lids::Half => self.half.as_ref().unwrap_or(&self.art.body),
            Lids::Closed => self.closed.as_ref().unwrap_or(&self.art.body),
        };
        canvas.blit(body, origin);

        if let (Some(spec), Some(mouth)) = (effects::fire_spec(self.stage), self.art.mouth) {
            if (FIRE_START..FIRE_END).contains(&t) {
                let progress = (t - FIRE_START) as f32 / (FIRE_END - FIRE_START - 1) as f32;
                effects::draw_fire(
                    &mut canvas,
                    spec,
                    (origin.0 + mouth.0, origin.1 + mouth.1),
                    progress,
                    t,
                );
            }
        }

        match self.stage {
            Stage::Legendary => effects::draw_magic(
                &mut canvas,
                (
                    origin.0,
                    origin.1,
                    self.art.body.w as i32,
                    self.art.body.h as i32,
                ),
                t,
                LOOP_FRAMES,
                self.magic,
            ),
            Stage::Egg => effects::draw_egg_glint(&mut canvas, origin, t, LOOP_FRAMES),
            _ => {}
        }
        canvas
    }

    /// All frames of the loop.
    pub fn loop_frames(&self) -> Vec<Frame> {
        (0..LOOP_FRAMES).map(|t| self.frame(t)).collect()
    }
}

/// Render the full loop for a stage.
pub fn render_loop(stage: Stage, coat: Option<&DragonCoat>) -> Result<Vec<Frame>, String> {
    Ok(Rig::new(stage, coat)?.loop_frames())
}

/// Render the still used for `dragon.png` and the share card (wings raised, eyes open).
pub fn render_still(stage: Stage, coat: Option<&DragonCoat>) -> Result<Frame, String> {
    Ok(Rig::new(stage, coat)?.frame(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(f: &Frame) -> usize {
        f.rgba.chunks(4).filter(|p| p[3] != 0).count()
    }

    #[test]
    fn every_stage_renders_inside_the_canvas() {
        for stage in Stage::ALL {
            let rig = Rig::new(stage, None).expect("rig");
            let (ox, oy) = rig.origin();
            assert!(ox >= 0 && oy >= 0, "{stage:?} origin {ox},{oy}");
            for t in [0, 10, 25, 45] {
                assert!(lit(&rig.frame(t)) > 500, "{stage:?} frame {t} nearly empty");
            }
        }
    }

    #[test]
    fn loop_is_seamless() {
        for stage in Stage::ALL {
            let rig = Rig::new(stage, None).expect("rig");
            assert!(
                rig.frame(0) == rig.frame(LOOP_FRAMES),
                "{stage:?} loop not seamless"
            );
        }
    }

    #[test]
    fn dragons_animate_and_the_egg_only_glints() {
        for stage in Stage::ALL {
            let frames = render_loop(stage, None).expect("frames");
            let distinct = frames.iter().filter(|f| **f != frames[0]).count();
            assert!(
                distinct > 8,
                "{stage:?} has only {distinct} frames different from frame 0"
            );
        }
    }

    #[test]
    fn rendering_is_reproducible() {
        let a = render_loop(Stage::Legendary, None).expect("a");
        let b = render_loop(Stage::Legendary, None).expect("b");
        assert!(a == b, "render is not reproducible");
    }

    #[test]
    fn blink_frames_differ_from_open_frames() {
        let rig = Rig::new(Stage::Adult, None).expect("rig");
        // Same wing phase (a multiple of the flap period), different eyelids.
        let period = LOOP_FRAMES / FLAPS_PER_LOOP;
        let open = rig.frame(22 + period);
        let closed = rig.frame(22);
        assert!(open != closed, "blink did not change the frame");
    }

    #[test]
    fn fire_only_appears_for_older_stages() {
        let adult = Rig::new(Stage::Adult, None).expect("rig");
        let baby = Rig::new(Stage::Baby, None).expect("rig");
        // Two frames with the same wing phase and open eyes, one before and one during the breath.
        let before = FIRE_START - LOOP_FRAMES / FLAPS_PER_LOOP;
        let during = before + 2 * (LOOP_FRAMES / FLAPS_PER_LOOP);
        assert!((FIRE_START..FIRE_END).contains(&during));
        assert!(lit(&adult.frame(during)) > lit(&adult.frame(before)) + 100);
        assert_eq!(lit(&baby.frame(during)), lit(&baby.frame(before)));
    }

    #[test]
    fn coat_changes_colours_but_not_the_silhouette() {
        let coat = DragonCoat::from_languages(&[crate::history::LanguageShare {
            name: "JavaScript".into(),
            hex_color: "#f1e05a".into(),
            repo_count: 5,
        }])
        .expect("coat");
        let plain = render_still(Stage::Adult, None).expect("plain");
        let tinted = render_still(Stage::Adult, Some(&coat)).expect("tinted");
        assert!(plain.rgba != tinted.rgba);
        let alpha = |f: &Frame| f.rgba.chunks(4).map(|p| p[3]).collect::<Vec<_>>();
        assert!(alpha(&plain) == alpha(&tinted));
    }
}
