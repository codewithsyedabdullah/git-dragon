/* ============================================================================
 * data.js — offline Linguist colours, demo snapshots and stage metadata.
 * Mirrors src/languages.rs and src/main.rs demo_snapshot() of git-dragon.
 * ========================================================================== */
(function () {
  'use strict';

  /* Linguist colours for the most common languages (name -> #rrggbb). */
  var LINGUIST = {
    'Assembly': '#6E4C13',
    'C': '#555555',
    'C#': '#178600',
    'C++': '#f34b7d',
    'CSS': '#663399',
    'Clojure': '#db5855',
    'Dart': '#00B4AB',
    'Dockerfile': '#384d54',
    'Elixir': '#6e4a7e',
    'Erlang': '#B83998',
    'F#': '#b845fc',
    'Go': '#00ADD8',
    'Groovy': '#e69f56',
    'HTML': '#e34c26',
    'Haskell': '#5e5086',
    'Java': '#b07219',
    'JavaScript': '#f1e05a',
    'Julia': '#a270ba',
    'Jupyter Notebook': '#DA5B0B',
    'Kotlin': '#A97BFF',
    'Lua': '#000080',
    'MATLAB': '#e16737',
    'Makefile': '#427819',
    'Nix': '#7e7eff',
    'OCaml': '#ef7a08',
    'Objective-C': '#438eff',
    'PHP': '#4F5D95',
    'Perl': '#0298c3',
    'PowerShell': '#012456',
    'Python': '#3572A5',
    'R': '#198CE7',
    'Ruby': '#701516',
    'Rust': '#dea584',
    'SCSS': '#c6538c',
    'Scala': '#c22d40',
    'Shell': '#89e051',
    'Svelte': '#ff3e00',
    'Swift': '#F05138',
    'TeX': '#3D6117',
    'TypeScript': '#3178c6',
    'Vue': '#41b883',
    'Zig': '#ec915c',
  };

  /* Deterministic, pleasant colour for an unknown language (FNV-1a -> HSV). */
  function hashedHex(name) {
    var hash = 0x811c9dc5;
    var lower = name.toLowerCase();
    for (var i = 0; i < lower.length; i++) {
      hash ^= lower.charCodeAt(i);
      hash = Math.imul(hash, 0x01000193);
    }
    var hue = (hash >>> 0) % 360;
    var rgb = hsvToRgb(hue, 0.55, 0.85);
    return '#' + rgb.map(function (c) {
      return c.toString(16).padStart(2, '0');
    }).join('');
  }

  function hsvToRgb(h, s, v) {
    var c = v * s;
    var x = c * (1 - Math.abs(((h / 60) % 2) - 1));
    var m = v - c;
    var q = Math.floor(h / 60);
    var r, g, b;
    switch (q) {
      case 0: r = c; g = x; b = 0; break;
      case 1: r = x; g = c; b = 0; break;
      case 2: r = 0; g = c; b = x; break;
      case 3: r = 0; g = x; b = c; break;
      case 4: r = x; g = 0; b = c; break;
      default: r = c; g = 0; b = x; break;
    }
    return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
  }

  /* Hex colour for a language name (case-insensitive). */
  function languageHex(name) {
    if (Object.prototype.hasOwnProperty.call(LINGUIST, name)) return LINGUIST[name];
    for (var key in LINGUIST) {
      if (key.toLowerCase() === name.trim().toLowerCase()) return LINGUIST[key];
    }
    return hashedHex(name.trim());
  }

  /* ------------------------------------------------------------------ *
   * Stage metadata for the UI                                           *
   * ------------------------------------------------------------------ */

  var STAGES = [
    {
      slug: 'egg', name: 'Dragon Egg', title: 'Stage 0 · Dragon Egg',
      lo: 0, hi: 0, fires: false,
      blurb: 'Green-and-cream shell that catches a slow glint. Every legend starts here.',
      tip: 'Make your first commit to hatch.',
    },
    {
      slug: 'baby', name: 'Baby Dragon', title: 'Stage 1 · Baby Dragon',
      lo: 1, hi: 149, fires: false,
      blurb: 'Big head, tiny horns and wings. Flutters its wings and blinks.',
      tip: 'Keep committing to grow.',
    },
    {
      slug: 'young', name: 'Young Dragon', title: 'Stage 2 · Young Dragon',
      lo: 150, hi: 499, fires: false,
      blurb: 'Longer body, wider wings, longer horns, back spikes.',
      tip: '500 contributions unlocks fire.',
    },
    {
      slug: 'adolescent', name: 'Adolescent Dragon', title: 'Stage 3 · Adolescent Dragon',
      lo: 500, hi: 1199, fires: true,
      blurb: 'Large veined wings, stronger legs — and a small fire breath.',
      tip: '1,200 contributions unlocks adulthood.',
    },
    {
      slug: 'adult', name: 'Adult Dragon', title: 'Stage 4 · Adult Dragon',
      lo: 1200, hi: 2499, fires: true,
      blurb: 'Full size, broad wings, long tail with a crest tip, bigger fire breath.',
      tip: '2,500 contributions unlocks legend.',
    },
    {
      slug: 'legendary', name: 'Legendary Dragon', title: 'Stage 5 · Legendary Dragon',
      lo: 2500, hi: null, fires: true,
      blurb: 'Glowing aura, rising embers, twinkling stars, golden wing edges, big fire breath.',
      tip: 'You are a legend. Keep going.',
    },
  ];

  /* ------------------------------------------------------------------ *
   * Bundled demo snapshots (offline renders)                            *
   * Same shape as git-dragon's AccountSnapshot; mirrors demo_snapshot(). *
   * ------------------------------------------------------------------ */

  function demoSnapshot(commits, languages) {
    var count = Math.max(languages.length, 1);
    return {
      login: 'demo-user',
      account_id: 1,
      observed_on: '2026-11-01',
      total_contributions: commits,
      monthly_contributions: Math.min(commits, 31),
      current_streak: 0,
      longest_streak: 0,
      languages: languages.map(function (name, i) {
        return { name: name, hex_color: languageHex(name), repo_count: count - i + 1 };
      }),
      source: 'demo',
    };
  }

  /* Demo dragons wear the untouched canonical emerald coat — language tinting
   * is only ever shown for a real, searched username. */
  var DEMO_LANGUAGES = [];

  var DEMO_SNAPSHOTS = {
    egg: demoSnapshot(0, DEMO_LANGUAGES),
    baby: demoSnapshot(120, DEMO_LANGUAGES),
    young: demoSnapshot(300, DEMO_LANGUAGES),
    adolescent: demoSnapshot(800, DEMO_LANGUAGES),
    adult: demoSnapshot(1800, DEMO_LANGUAGES),
    legendary: demoSnapshot(3200, DEMO_LANGUAGES),
  };

  window.DragonData = {
    LINGUIST: LINGUIST,
    languageHex: languageHex,
    STAGES: STAGES,
    DEMO_SNAPSHOTS: DEMO_SNAPSHOTS,
  };
})();
