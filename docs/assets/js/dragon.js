/* ============================================================================
 * dragon.js — faithful browser port of git-dragon's Rust renderer.
 *
 * Ports: coat.rs (DragonCoat/Tint/ramp/readable), sprite.rs (Layer, Canvas,
 * blit, blit_rotated, recolor, with_lids), effects.rs (fire, magic, egg glint),
 * render.rs (48-frame loop, wing flap, lids, origins), stage.rs (thresholds).
 *
 * Everything is a pure function of (stage, coat, frame index) — same inputs,
 * same pixels as the Rust binary. Rust's f32 arithmetic is emulated with
 * Math.fround so colour-threshold and pixel-position decisions match exactly.
 * ========================================================================== */
(function () {
  'use strict';

  var CANVAS_W = 640;
  var CANVAS_H = 300;
  var LOOP_FRAMES = 48;
  var GROUND_MARGIN = 8;
  var FIRE_START = 26;
  var FIRE_END = 46;
  var ALPHA_CUTOFF = 128;
  var OUTLINE = [18, 32, 30];

  /* Flame colours, hottest to coolest (effects.rs). */
  var FIRE_PALE = [255, 244, 170];
  var FIRE_YELLOW = [255, 200, 40];
  var FIRE_ORANGE = [255, 130, 20];
  var FIRE_RED = [215, 60, 10];
  var FIRE_EMBER = [140, 28, 8];
  var BLOCK = 2;

  /* Soft warm covering around the flame: halo rings in block units with a
   * blend alpha, plus how far the halo colour is dimmed towards black. */
  var FIRE_HALO = [
    [-1, 0, 0.40], [1, 0, 0.40], [0, -1, 0.40], [0, 1, 0.40],
    [-1, -1, 0.22], [1, -1, 0.22], [-1, 1, 0.22], [1, 1, 0.22],
    [-2, 0, 0.14], [2, 0, 0.14], [0, -2, 0.14], [0, 2, 0.14],
    [-2, -2, 0.07], [2, -2, 0.07], [-2, 2, 0.07], [2, 2, 0.07],
  ];
  var GLOW_DIM = 0.45;

  /* ------------------------------------------------------------------ *
   * Stage metadata (stage.rs)                                           *
   * ------------------------------------------------------------------ */

  var STAGE_ORDER = ['egg', 'baby', 'young', 'adolescent', 'adult', 'legendary'];

  var STAGE_NAMES = {
    egg: 'Dragon Egg',
    baby: 'Baby Dragon',
    young: 'Young Dragon',
    adolescent: 'Adolescent Dragon',
    adult: 'Adult Dragon',
    legendary: 'Legendary Dragon',
  };

  /* Inclusive contribution ranges; null upper bound = open-ended. */
  var STAGE_RANGES = {
    egg: [0, 0],
    baby: [1, 149],
    young: [150, 499],
    adolescent: [500, 1199],
    adult: [1200, 2499],
    legendary: [2500, null],
  };

  function stageFromContributions(n) {
    if (n === 0) return 'egg';
    if (n <= 149) return 'baby';
    if (n <= 499) return 'young';
    if (n <= 1199) return 'adolescent';
    if (n <= 2499) return 'adult';
    return 'legendary';
  }

  function stageIndex(slug) { return STAGE_ORDER.indexOf(slug); }

  function breathesFire(slug) {
    return slug === 'adolescent' || slug === 'adult' || slug === 'legendary';
  }

  /* Wing swing amplitude around the resting angle, degrees (render.rs). */
  var WING_AMP = { egg: 0, baby: 14, young: 16, adolescent: 18, adult: 20, legendary: 22 };

  /* Fire breath size per stage (effects.rs). */
  var FIRE_SPEC = {
    adolescent: { length: 80, radius: 14 },
    adult: { length: 115, radius: 18 },
    legendary: { length: 140, radius: 22 },
  };

  /* ------------------------------------------------------------------ *
   * Sprite metadata (sprite.rs): layers, pivots, eyes, mouths           *
   * ------------------------------------------------------------------ */

  var SPRITE_BASE = 'assets/dragons/';

  var ART = {
    egg: {
      body: 'egg.png', wing: null, root: null,
      pivot: [0, 0], eye: null, mouth: null,
    },
    baby: {
      body: 'baby_body.png', wing: 'baby_wing.png', root: 'baby_root.png',
      pivot: [82, 68], eye: { x0: 26, y0: 42, x1: 44, y1: 58 }, mouth: null,
    },
    young: {
      body: 'young_body.png', wing: 'young_wing.png', root: 'young_root.png',
      pivot: [95, 95], eye: { x0: 27, y0: 49, x1: 47, y1: 66 }, mouth: null,
    },
    adolescent: {
      body: 'adolescent_body.png', wing: 'adolescent_wing.png', root: 'adolescent_root.png',
      pivot: [100, 95], eye: { x0: 30, y0: 47, x1: 48, y1: 65 }, mouth: [1, 69],
    },
    adult: {
      body: 'adult_body.png', wing: 'adult_wing.png', root: 'adult_root.png',
      pivot: [112, 105], eye: { x0: 32, y0: 61, x1: 59, y1: 81 }, mouth: [1, 87],
    },
    legendary: {
      body: 'legendary_body.png', wing: 'legendary_wing.png', root: 'legendary_root.png',
      pivot: [138, 115], eye: { x0: 58, y0: 82, x1: 77, y1: 100 }, mouth: [34, 113],
    },
  };

  /* ------------------------------------------------------------------ *
   * Small helpers                                                       *
   * ------------------------------------------------------------------ */

  function clamp(v, lo, hi) { return v < lo ? lo : v > hi ? hi : v; }
  function eucMod(x, m) { return ((x % m) + m) % m; }
  /* Rust `as i32` on f32 truncates toward zero (NOT floor for negatives). */
  function trunc(x) { return Math.trunc(x); }

  /* Rust does all colour/effect math in f32. Emulate it with Math.fround so
   * threshold comparisons (hue = 70, saturation = 0.12, ...) and pixel
   * positions land on exactly the same side as the Rust binary. Every
   * intermediate is rounded and every fractional literal is the f32 value. */
  var F = Math.fround;
  var F32_EPSILON = 1.1920928955078125e-7; // f32::EPSILON
  var F32_PI = F(Math.PI); // std::f32::consts::PI
  var F32_TAU = F(2 * Math.PI); // std::f32::consts::TAU
  var C299 = F(0.299), C587 = F(0.587), C114 = F(0.114);

  /* ------------------------------------------------------------------ *
   * Layer / Canvas (sprite.rs)                                          *
   * ------------------------------------------------------------------ */

  function Layer(w, h, data) {
    this.w = w;
    this.h = h;
    this.data = data; // Uint8ClampedArray RGBA
  }

  Layer.prototype.px = function (x, y) {
    var i = (y * this.w + x) * 4;
    return [this.data[i], this.data[i + 1], this.data[i + 2], this.data[i + 3]];
  };

  function Canvas(w, h) {
    this.w = w;
    this.h = h;
    this.data = new Uint8ClampedArray(w * h * 4);
  }

  Canvas.prototype.put = function (x, y, rgb) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    var i = (y * this.w + x) * 4;
    this.data[i] = rgb[0];
    this.data[i + 1] = rgb[1];
    this.data[i + 2] = rgb[2];
    this.data[i + 3] = 255;
  };

  Canvas.prototype.block = function (x, y, size, rgb) {
    for (var dy = 0; dy < size; dy++) {
      for (var dx = 0; dx < size; dx++) this.put(x + dx, y + dy, rgb);
    }
  };

  /* Alpha-blend one pixel over the canvas (standard "over" compositing) —
   * used for the fire halo. Mirrors Canvas::put_blend in the Rust renderer. */
  Canvas.prototype.putBlend = function (x, y, rgb, a) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    var i = (y * this.w + x) * 4;
    var da = this.data[i + 3] / 255;
    var oa = a + da * (1 - a);
    if (oa <= 0) return;
    var dw = da * (1 - a);
    this.data[i] = (rgb[0] * a + this.data[i] * dw) / oa;
    this.data[i + 1] = (rgb[1] * a + this.data[i + 1] * dw) / oa;
    this.data[i + 2] = (rgb[2] * a + this.data[i + 2] * dw) / oa;
    this.data[i + 3] = oa * 255;
  };

  Canvas.prototype.blockBlend = function (x, y, size, rgb, a) {
    for (var dy = 0; dy < size; dy++) {
      for (var dx = 0; dx < size; dx++) this.putBlend(x + dx, y + dy, rgb, a);
    }
  };

  Canvas.prototype.blit = function (layer, origin) {
    for (var sy = 0; sy < layer.h; sy++) {
      for (var sx = 0; sx < layer.w; sx++) {
        var i = (sy * layer.w + sx) * 4;
        if (layer.data[i + 3] >= ALPHA_CUTOFF) {
          this.put(origin[0] + sx, origin[1] + sy, [layer.data[i], layer.data[i + 1], layer.data[i + 2]]);
        }
      }
    }
  };

  /* Draw `layer` rotated clockwise by `deg` and scaled about `pivot`.
   * Nearest-neighbour sampling keeps the pixel art crisp. f32 arithmetic
   * throughout, mirroring blit_rotated — the wing-edge floor() calls flip
   * pixels otherwise. */
  Canvas.prototype.blitRotated = function (layer, origin, pivot, deg, scale) {
    var rad = F(F(deg * F32_PI) / 180);
    var sn = F(Math.sin(rad));
    var c = F(Math.cos(rad));
    var pcx = F(origin[0] + pivot[0]);
    var pcy = F(origin[1] + pivot[1]);

    var corners = [[0, 0], [layer.w, 0], [0, layer.h], [layer.w, layer.h]];
    var xMin = Infinity, yMin = Infinity, xMax = -Infinity, yMax = -Infinity;
    for (var k = 0; k < 4; k++) {
      var cx = corners[k][0], cy = corners[k][1];
      var dx = F(cx - pivot[0]);
      var dy = F(cy - pivot[1]);
      var x = F(pcx + F(scale * F(F(c * dx) - F(sn * dy))));
      var y = F(pcy + F(scale * F(F(sn * dx) + F(c * dy))));
      if (x < xMin) xMin = x;
      if (y < yMin) yMin = y;
      if (x > xMax) xMax = x;
      if (y > yMax) yMax = y;
    }
    var x0 = Math.max(Math.floor(xMin) - 2, 0);
    var y0 = Math.max(Math.floor(yMin) - 2, 0);
    var x1 = Math.min(Math.ceil(xMax) + 2, this.w);
    var y1 = Math.min(Math.ceil(yMax) + 2, this.h);

    for (var y = y0; y < y1; y++) {
      for (var x = x0; x < x1; x++) {
        var ddx = F(F(x + 0.5) - pcx);
        var ddy = F(F(y + 0.5) - pcy);
        var sx = Math.floor(F(F(F(c * ddx) + F(sn * ddy)) / scale + pivot[0]));
        var sy = Math.floor(F(F(F(-sn * ddx) + F(c * ddy)) / scale + pivot[1]));
        if (sx < 0 || sy < 0 || sx >= layer.w || sy >= layer.h) continue;
        var i = (sy * layer.w + sx) * 4;
        if (layer.data[i + 3] >= ALPHA_CUTOFF) {
          this.put(x, y, [layer.data[i], layer.data[i + 1], layer.data[i + 2]]);
        }
      }
    }
  };

  /* ------------------------------------------------------------------ *
   * Language coat (coat.rs)                                             *
   * ------------------------------------------------------------------ */

  function parseHexColor(hex) {
    if (typeof hex !== 'string') return null;
    var m = /^#?([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})$/.exec(hex.trim());
    if (!m) return null;
    return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
  }

  function rampFromRgb(base) {
    var b0 = base[0], b1 = base[1], b2 = base[2];
    var mul = function (k) { return [F(b0 * k), F(b1 * k), F(b2 * k)]; };
    var lift = function (k) {
      return [
        F(b0 + F(F(255 - b0) * k)),
        F(b1 + F(F(255 - b1) * k)),
        F(b2 + F(F(255 - b2) * k)),
      ];
    };
    return [mul(F(0.22)), mul(F(0.55)), [b0, b1, b2], lift(F(0.5))];
  }

  /* Lighten colours dark enough to swallow the dragon's dark outlines. */
  function readable(base) {
    var luma = F(F(F(C299 * base[0]) + F(C587 * base[1])) + F(C114 * base[2]));
    var MIN_LUMA = 80;
    if (luma >= MIN_LUMA) return base;
    var k = clamp(F(F(MIN_LUMA - luma) / F(255 - luma)), 0, 1);
    return base.map(function (c) { return Math.round(F(F(c) + F(F(255 - c) * k))); });
  }

  function makeTint(language, base, strength) {
    base = readable(base);
    return {
      language: language,
      base: base,
      ramp: rampFromRgb(base),
      strength: clamp(strength, 0, 1),
    };
  }

  /* Build a coat from languages ordered most-used first.
   * Returns null when nothing usable is left (canonical emerald dragon). */
  function coatFromLanguages(languages) {
    var usable = [];
    for (var i = 0; i < languages.length && usable.length < 3; i++) {
      var rgb = parseHexColor(languages[i].hex_color);
      if (rgb) usable.push({ lang: languages[i], rgb: rgb });
    }
    if (!usable.length) return null;

    var last = usable[usable.length - 1];
    var weights = usable.map(function (u) { return Math.max(u.lang.repo_count, 1); });
    var total = 0;
    for (var w = 0; w < weights.length; w++) total = F(total + weights[w]);
    var share = function (i) {
      var wt = i < weights.length ? weights[i] : weights[weights.length - 1];
      return F(wt / total);
    };
    var pick = function (i) { return i < usable.length ? usable[i] : last; };

    var p0 = pick(0), p1 = pick(1), p2 = pick(2);
    return {
      primary: makeTint(p0.lang.name, p0.rgb, F(F(0.55) + F(F(0.4) * share(0)))),
      secondary: makeTint(p1.lang.name, p1.rgb, F(F(0.45) + F(F(0.5) * share(1)))),
      tertiary: makeTint(p2.lang.name, p2.rgb, F(F(0.5) + F(F(0.45) * share(2)))),
    };
  }

  function coatKey(coat) {
    if (!coat) return 'canon';
    return [coat.primary, coat.secondary, coat.tertiary]
      .map(function (t) { return t.language + ':' + t.base.join(',') + ':' + t.strength; })
      .join('|');
  }

  /* ------------------------------------------------------------------ *
   * Recolouring (sprite.rs)                                             *
   * ------------------------------------------------------------------ */

  function rgbToHsv(r, g, b) {
    r = F(r / 255); g = F(g / 255); b = F(b / 255);
    var mx = Math.max(r, g, b);
    var mn = Math.min(r, g, b);
    var d = F(mx - mn);
    var h = 0;
    if (d < F(1e-6)) {
      h = 0;
    } else if (Math.abs(mx - r) < F32_EPSILON) {
      h = eucMod(F(F(g - b) / d), 6);
    } else if (Math.abs(mx - g) < F32_EPSILON) {
      h = F(F(F(b - r) / d) + 2);
    } else {
      h = F(F(F(r - g) / d) + 4);
    }
    return [F(h * 60), mx > 0 ? F(d / mx) : 0, mx];
  }

  function sampleRamp(ramp, t) {
    t = F(clamp(t, 0, 1) * 3);
    var i = Math.min(Math.floor(t), 2);
    var f = F(t - i);
    var out = [0, 0, 0];
    for (var c = 0; c < 3; c++) {
      out[c] = F(F(ramp[i][c] * F(1 - f)) + F(ramp[i + 1][c] * f));
    }
    return out;
  }

  function lerp3(a, b, k) {
    return [
      F(a[0] + F((b[0] - a[0]) * k)),
      F(a[1] + F((b[1] - a[1]) * k)),
      F(a[2] + F((b[2] - a[2]) * k)),
    ];
  }

  /* Re-tint a layer while keeping its shading: brightness picks the position
   * on the ramp. Green/teal/blue -> primary (secondary on the wing); cream in
   * the top third (horns) -> tertiary highlight, elsewhere -> primary
   * highlight; eyes, whites and the dark outline are left alone. */
  function recolorLayer(layer, coat, part, root) {
    var data = layer.data;
    var hornLimit = F(F(0.3) * layer.h);
    for (var y = 0; y < layer.h; y++) {
      for (var x = 0; x < layer.w; x++) {
        var i = (y * layer.w + x) * 4;
        if (data[i + 3] === 0) continue;
        var or_ = data[i], og = data[i + 1], ob = data[i + 2];
        var hsv = rgbToHsv(or_, og, ob);
        var h = hsv[0], s = hsv[1], v = hsv[2];
        var luma = F(F(F(F(C299 * or_) + F(C587 * og)) + F(C114 * ob)) / 255);
        var t = clamp(F(F(luma - F(0.07)) / F(0.78)), 0, 1);
        var green = h >= 70 && h <= 260 && s >= F(0.12) && v >= F(0.05);
        var warm = h >= 15 && h < 70 && s >= F(0.1) && v >= F(0.55);
        var inRoot = root && root.data[i + 3] !== 0;
        var wing = part === 'wing' || inRoot;

        var target = null;
        var strength = 0;
        if (green || (wing && warm)) {
          var tint = wing ? coat.secondary : coat.primary;
          target = sampleRamp(tint.ramp, t);
          strength = tint.strength;
        } else if (warm) {
          if (y < hornLimit) {
            target = lerp3([or_, og, ob], coat.tertiary.ramp[3], F(0.6));
            strength = coat.tertiary.strength;
          } else {
            target = lerp3([or_, og, ob], coat.primary.ramp[3], F(0.4));
            strength = coat.primary.strength;
          }
        }

        if (target) {
          var out = lerp3([or_, og, ob], target, strength);
          data[i] = clamp(Math.round(out[0]), 0, 255);
          data[i + 1] = clamp(Math.round(out[1]), 0, 255);
          data[i + 2] = clamp(Math.round(out[2]), 0, 255);
        }
      }
    }
  }

  /* ------------------------------------------------------------------ *
   * Blinking (sprite.rs)                                                *
   * ------------------------------------------------------------------ */

  function withLids(body, eye, lids) {
    var out = body.data.slice();
    if (lids === 'open') return { w: body.w, h: body.h, data: out };

    var w = eye.x1 - eye.x0;
    var h = eye.y1 - eye.y0;
    var cy = eye.y0 + (h >> 1);

    /* Skin colour: first clearly-green opaque pixel to the left of the eye. */
    var skin = null;
    for (var dx = 2; dx <= 8; dx++) {
      var sx = Math.max(eye.x0 - dx, 0);
      var p = body.px(sx, cy);
      if (p[3] >= ALPHA_CUTOFF && p[1] > Math.min(p[0] + 12, 255) && p[1] > 70) {
        skin = [p[0], p[1], p[2]];
        break;
      }
    }
    if (!skin) return { w: body.w, h: body.h, data: out };

    var coverTo = lids === 'half' ? eye.y0 + (h >> 1) : eye.y1;
    for (var y = eye.y0; y < Math.min(coverTo, body.h); y++) {
      for (var x = eye.x0; x < Math.min(eye.x1, body.w); x++) {
        var pp = body.px(x, y);
        if (pp[3] < ALPHA_CUTOFF) continue;
        var luma = F(F(F(C299 * pp[0]) + F(C587 * pp[1])) + F(C114 * pp[2]));
        var nearWhite = Math.min(pp[0], pp[1], pp[2]) > 200;
        var warm = pp[0] > Math.min(pp[1] + 12, 255);
        if (luma < 110 || nearWhite || warm) {
          var i = (y * body.w + x) * 4;
          out[i] = skin[0];
          out[i + 1] = skin[1];
          out[i + 2] = skin[2];
        }
      }
    }

    /* Eyelid line. */
    var lineY = lids === 'half' ? eye.y0 + (h >> 1) : eye.y0 + (((h * 6) / 10) | 0);
    var thickness = h >= 16 ? 2 : 1;
    var xEnd = Math.max(eye.x1 - ((w >> 3) + 1), 0);
    for (var dy = 0; dy < thickness; dy++) {
      var ly = lineY + dy;
      if (ly >= body.h) continue;
      for (var lx = eye.x0 + 1; lx < xEnd; lx++) {
        var j = (ly * body.w + lx) * 4;
        out[j] = OUTLINE[0];
        out[j + 1] = OUTLINE[1];
        out[j + 2] = OUTLINE[2];
        out[j + 3] = 255;
      }
    }
    return { w: body.w, h: body.h, data: out };
  }

  /* ------------------------------------------------------------------ *
   * Effects (effects.rs)                                                *
   * ------------------------------------------------------------------ */

  /* Small deterministic hash -> 0.0..1.0 (f32 division, as in Rust). */
  function noise(a, b, c) {
    var h = (Math.imul(a | 0, 0x9e3779b1) ^ Math.imul(b | 0, 0x85ebca6b) ^ Math.imul(c | 0, 0xc2b2ae35)) >>> 0;
    h ^= h >>> 15;
    h = Math.imul(h, 0x2c1b3c6d) >>> 0;
    h ^= h >>> 12;
    return F((h & 0xffff) / 65535);
  }

  /* Flame leaving the mouth to the left. progress 0..1, tick = frame index.
   * f32 chain as in effects.rs; powf is the one accepted f64 approximation
   * (it can only jitter a flame edge by a pixel, never move a whole block).
   * A soft warm covering (dim halo rings) is blended around the flame sheet
   * first, then the hot core is drawn opaque on top. */
  function drawFire(canvas, spec, mouth, progress, tick) {
    var envelope = F(Math.pow(F(Math.sin(F(F(clamp(progress, 0, 1)) * F32_PI))), F(0.6)));
    var length = F(spec.length * envelope);
    if (length < 8) return;
    var steps = trunc(F(length / 2));
    var blocks = [];
    for (var i = 0; i < steps; i++) {
      var d = F(i * 2);
      var f = F(d / length);
      var half = F(F(F(spec.radius * envelope) * (F(F(0.3) + F(F(0.7) * F(Math.pow(f, F(0.8))))))));
      var centre = F(F(-d * F(0.1)) + F(3 * F(Math.sin(F(F(d * F(0.11)) - F(tick * F(0.55)))))));
      var rows = Math.ceil(F(half / 2));
      for (var j = -rows; j <= rows; j++) {
        var dy = F(j * 2);
        var n = half > 0 ? F(Math.abs(dy) / half) : 1;
        var jitter = noise(i, j, (tick / 2) | 0);
        var heat = F(F(F(1 - F(F(0.75) * f)) - F(F(0.55) * n)) + F(F(F(0.35) * F(jitter - F(0.5)))));
        if (heat < F(0.08)) continue;
        var rgb = heat > 0.75 ? FIRE_PALE
          : heat > 0.5 ? FIRE_YELLOW
          : heat > 0.3 ? FIRE_ORANGE
          : heat > 0.15 ? FIRE_RED
          : FIRE_EMBER;
        var x = mouth[0] - i * 2 - 2;
        var y = mouth[1] + trunc(F(centre + dy)) - 1;
        blocks.push([x, y, rgb]);
      }
    }
    /* covering: dim warm halo rings around every flame block */
    for (var b = 0; b < blocks.length; b++) {
      var bx = blocks[b][0], by = blocks[b][1], bc = blocks[b][2];
      var glow = [Math.round(bc[0] * GLOW_DIM), Math.round(bc[1] * GLOW_DIM), Math.round(bc[2] * GLOW_DIM)];
      for (var h = 0; h < FIRE_HALO.length; h++) {
        var o = FIRE_HALO[h];
        canvas.blockBlend(bx + o[0] * BLOCK, by + o[1] * BLOCK, BLOCK, glow, o[2]);
      }
    }
    /* hot core on top */
    for (var c = 0; c < blocks.length; c++) {
      canvas.block(blocks[c][0], blocks[c][1], BLOCK, blocks[c][2]);
    }
  }

  /* A `+` shaped twinkle. */
  function star(canvas, x, y, arm, colour) {
    canvas.block(x - 1, y - 1, BLOCK, [255, 255, 255]);
    for (var k = 1; k <= arm; k++) {
      var o = k * BLOCK;
      canvas.block(x - 1 + o, y - 1, BLOCK, colour);
      canvas.block(x - 1 - o, y - 1, BLOCK, colour);
      canvas.block(x - 1, y - 1 + o, BLOCK, colour);
      canvas.block(x - 1, y - 1 - o, BLOCK, colour);
    }
  }

  /* Rising embers and twinkling stars around the legendary dragon. */
  function drawMagic(canvas, area, tick, frames, colours) {
    var phase = F(tick / frames);
    var ax = area[0], ay = area[1], aw = area[2], ah = area[3];

    var span = F(Math.max(F(F(ah) * F(0.9)), 1));
    for (var k = 0; k < 18; k++) {
      var bx = F(F(ax + 10) + F(noise(k, 1, 7) * F(aw - 20)));
      var start = F(noise(k, 2, 7) * span);
      var lift = F(eucMod(F(start - F(F(phase * span) * 2)), span));
      var y = F(F(F(ay + ah) - 6) - lift);
      var x = F(bx + F(3 * F(Math.sin(F(F32_TAU * F(phase + noise(k, 3, 7)))))));
      var fade = F(lift / span);
      if (fade > F(0.92)) continue;
      var size = fade > F(0.6) ? 1 : 2;
      var rgb = k % 3 === 0 ? [210, 255, 245] : colours.ember;
      canvas.block(trunc(x), trunc(y), size, rgb);
    }

    for (var s = 0; s < 6; s++) {
      var sx = ax + 8 + trunc(F(noise(s, 4, 9) * F(aw - 16)));
      var sy = ay + 4 + trunc(F(noise(s, 5, 9) * F(F(ah) * F(0.55))));
      var sv = F(Math.sin(F(F32_TAU * F(F(phase * 2) + noise(s, 6, 9)))));
      if (sv > F(0.25)) {
        var arm = 1 + trunc(F(sv * 3));
        star(canvas, sx, sy, arm, colours.spark);
      }
    }
  }

  /* A slow glint on the egg shell. */
  function drawEggGlint(canvas, origin, tick, frames) {
    var s = F(Math.sin(F(F(F32_TAU * tick) / frames)));
    if (s > F(0.2)) {
      var arm = 1 + trunc(F(s * 2));
      star(canvas, origin[0] + 26, origin[1] + 30, arm, [255, 250, 215]);
    }
  }

  /* ------------------------------------------------------------------ *
   * Eyelid timeline (render.rs)                                         *
   * ------------------------------------------------------------------ */

  function lidsAt(t) {
    t = t % LOOP_FRAMES;
    if (t === 5 || t === 6 || t === 22) return 'closed';
    if (t === 4 || t === 7 || t === 21 || t === 23) return 'half';
    return 'open';
  }

  /* ------------------------------------------------------------------ *
   * Sprite loading                                                      *
   * ------------------------------------------------------------------ */

  var artCache = {};
  var layerCache = {};

  function loadLayer(url) {
    if (layerCache[url]) return layerCache[url];
    /* Optional cache-bust hook (used by the local pixel-diff harness). */
    var bust = (typeof window !== 'undefined' && window.__DRAGON_BUST) || '';
    var p = fetch(url + bust).then(function (r) {
      if (!r.ok) throw new Error('HTTP ' + r.status + ' for ' + url);
      return r.blob();
    }).then(function (blob) {
      /* Straight-alpha decode, no colour-space transforms: byte-exact sprites. */
      return createImageBitmap(blob, { colorSpaceConversion: 'none', premultiplyAlpha: 'none' });
    }).then(function (bmp) {
      var c = document.createElement('canvas');
      c.width = bmp.width;
      c.height = bmp.height;
      var ctx = c.getContext('2d', { willReadFrequently: true });
      ctx.drawImage(bmp, 0, 0);
      var id = ctx.getImageData(0, 0, bmp.width, bmp.height);
      return new Layer(bmp.width, bmp.height, id.data);
    });
    layerCache[url] = p;
    return p;
  }

  function loadArt(slug) {
    if (artCache[slug]) return Promise.resolve(artCache[slug]);
    var meta = ART[slug];
    var bodyUrl = SPRITE_BASE + meta.body;
    return loadLayer(bodyUrl).then(function (body) {
      var art = { body: body, wing: null, root: null, pivot: meta.pivot, eye: meta.eye, mouth: meta.mouth };
      if (!meta.wing) {
        artCache[slug] = art;
        return art;
      }
      return Promise.all([
        loadLayer(SPRITE_BASE + meta.wing),
        loadLayer(SPRITE_BASE + meta.root),
      ]).then(function (parts) {
        art.wing = parts[0];
        art.root = parts[1];
        artCache[slug] = art;
        return art;
      });
    });
  }

  function preloadAll(onProgress) {
    var urls = [];
    STAGE_ORDER.forEach(function (slug) {
      var m = ART[slug];
      urls.push(m.body);
      if (m.wing) { urls.push(m.wing); urls.push(m.root); }
    });
    var done = 0;
    /* loadArt shares the layer cache, so this single pass primes everything. */
    return Promise.all(urls.map(function (u) {
      return loadLayer(SPRITE_BASE + u).then(function () {
        done++;
        if (onProgress) onProgress(done, urls.length);
      });
    })).then(function () {
      return Promise.all(STAGE_ORDER.map(function (slug) { return loadArt(slug); }));
    }).then(function () { return true; });
  }

  /* ------------------------------------------------------------------ *
   * Rig (render.rs): tinted art + precomputed eyelid variants           *
   * ------------------------------------------------------------------ */

  function buildRig(slug, coat) {
    var meta = ART[slug];
    var pristine = artCache[slug];
    if (!pristine) throw new Error('art not loaded: ' + slug);

    var body = new Layer(pristine.body.w, pristine.body.h, pristine.body.data.slice());
    var wing = null;
    var root = null;
    if (pristine.wing) {
      wing = new Layer(pristine.wing.w, pristine.wing.h, pristine.wing.data.slice());
      root = new Layer(pristine.root.w, pristine.root.h, pristine.root.data.slice());
    }

    var magic = { ember: [96, 240, 214], spark: [255, 226, 120] };
    if (coat) {
      recolorLayer(body, coat, 'body', root);
      if (wing) recolorLayer(wing, coat, 'wing', null);
      magic.ember = coat.secondary.ramp[3].map(function (c) { return clamp(Math.round(c), 0, 255); });
      magic.spark = coat.tertiary.ramp[3].map(function (c) { return clamp(Math.round(c), 0, 255); });
    }

    var half = null;
    var closed = null;
    if (meta.eye) {
      half = withLids(body, meta.eye, 'half');
      closed = withLids(body, meta.eye, 'closed');
    }

    return {
      slug: slug,
      body: body,
      wing: wing,
      root: root,
      pivot: meta.pivot,
      eye: meta.eye,
      mouth: meta.mouth,
      half: half,
      closed: closed,
      magic: magic,
      amp: WING_AMP[slug],
      fire: FIRE_SPEC[slug] || null,
    };
  }

  function rigOrigin(rig) {
    var ox = Math.floor((CANVAS_W - rig.body.w) / 2);
    var oy = CANVAS_H - rig.body.h - GROUND_MARGIN;
    return [ox, oy];
  }

  /* Render frame `t` (any value; wraps around the loop). */
  function renderFrame(rig, t) {
    t = t % LOOP_FRAMES;
    var canvas = new Canvas(CANVAS_W, CANVAS_H);
    var origin = rigOrigin(rig);

    if (rig.wing) {
      var ph = F(F(F(F32_TAU * 3) * t) / LOOP_FRAMES);
      var sn = F(Math.sin(ph));
      var angle = F(8 + F(rig.amp * sn));
      var scale = F(1 - F(F(0.07) * F(Math.max(sn, 0))));
      canvas.blitRotated(rig.wing, origin, rig.pivot, angle, scale);
    }

    var lids = lidsAt(t);
    var body = lids === 'open' ? rig.body : (lids === 'half' ? rig.half : rig.closed);
    canvas.blit(body, origin);

    if (rig.fire && rig.mouth && t >= FIRE_START && t < FIRE_END) {
      var progress = F(F(t - FIRE_START) / F(FIRE_END - FIRE_START - 1));
      drawFire(canvas, rig.fire, [origin[0] + rig.mouth[0], origin[1] + rig.mouth[1]], progress, t);
    }

    if (rig.slug === 'legendary') {
      drawMagic(canvas, [origin[0], origin[1], rig.body.w, rig.body.h], t, LOOP_FRAMES, rig.magic);
    } else if (rig.slug === 'egg') {
      drawEggGlint(canvas, origin, t, LOOP_FRAMES);
    }
    return canvas;
  }

  /* ------------------------------------------------------------------ *
   * Public renderer                                                     *
   * ------------------------------------------------------------------ */

  var rigCache = new Map();

  function getRig(slug, coat) {
    var key = slug + '|' + coatKey(coat);
    var rig = rigCache.get(key);
    if (!rig) {
      rig = buildRig(slug, coat);
      rigCache.set(key, rig);
    }
    return rig;
  }

  /* Paint a rig frame onto a visible 640x300 canvas element. */
  function paintFrame(canvasEl, rig, t) {
    var frame = renderFrame(rig, t);
    var ctx = canvasEl.getContext('2d');
    var img = ctx.createImageData(CANVAS_W, CANVAS_H);
    img.data.set(frame.data);
    ctx.putImageData(img, 0, 0);
  }

  /* Render a still (frame 0: wings raised, eyes open) to an offscreen canvas. */
  function stillCanvas(rig) {
    var c = document.createElement('canvas');
    c.width = CANVAS_W;
    c.height = CANVAS_H;
    var frame = renderFrame(rig, 0);
    var ctx = c.getContext('2d');
    var img = ctx.createImageData(CANVAS_W, CANVAS_H);
    img.data.set(frame.data);
    ctx.putImageData(img, 0, 0);
    return c;
  }

  /* Share card: still on a dark 1280x640 card, scaled 2x, hard pixel edges
   * (output.rs share_card). */
  function shareCardCanvas(rig) {
    var W = 1280, H = 640;
    var card = document.createElement('canvas');
    card.width = W;
    card.height = H;
    var ctx = card.getContext('2d');
    ctx.fillStyle = '#0d1117';
    ctx.fillRect(0, 0, W, H);
    var still = stillCanvas(rig);
    var scale = 2;
    var offX = Math.floor((W - still.width * scale) / 2);
    var offY = Math.floor((H - still.height * scale) / 2);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(still, offX, offY, still.width * scale, still.height * scale);
    return card;
  }

  window.DragonRenderer = {
    CANVAS_W: CANVAS_W,
    CANVAS_H: CANVAS_H,
    LOOP_FRAMES: LOOP_FRAMES,
    STAGE_ORDER: STAGE_ORDER,
    STAGE_NAMES: STAGE_NAMES,
    STAGE_RANGES: STAGE_RANGES,
    stageFromContributions: stageFromContributions,
    stageIndex: stageIndex,
    breathesFire: breathesFire,
    coatFromLanguages: coatFromLanguages,
    preload: preloadAll,
    rig: getRig,
    renderFrame: renderFrame,
    paintFrame: paintFrame,
    stillCanvas: stillCanvas,
    shareCardCanvas: shareCardCanvas,
  };
})();
