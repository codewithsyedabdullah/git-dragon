# Rendering Architecture

```
AccountSnapshot -> Stage + DragonCoat -> Rig (art, tint, eyelids) -> frames -> GIF / PNG / share card / HTML
```

- **Art** (`sprite.rs`, `assets/dragons/`): per stage a body layer, a wing layer and a small "wing root" mask, all RGBA PNGs embedded with `include_bytes!`. The wing is a separate layer so it can swing around the shoulder pivot.
- **Tinting** (`sprite.rs`): `Art::tinted` recolours every layer by brightness using the coat ramps. The wing root in the body layer follows the wing colour.
- **Frames** (`render.rs`): draw order is wing (rotated, nearest-neighbour), body (with the right eyelid variant), fire, then legendary embers/stars or the egg glint. Frame `t` depends only on `t`, so loops are seamless and output is reproducible.
- **Effects** (`effects.rs`): fire, embers and stars are pure functions of the frame index and a small integer hash. No randomness.
- **Encoding** (`output.rs`): one palette is shared by every frame (exact colours when 255 or fewer, NeuQuant otherwise), frames are cropped to their drawn area, and the GIF loops forever with 1-bit transparency. The share card places the still at 2x on a dark 1280x640 card.

Canvas: 640x300 for every stage so growth is visible when the GIF is embedded at a fixed width. Loop: 48 frames at 70 ms.
